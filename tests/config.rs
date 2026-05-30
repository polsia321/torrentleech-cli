use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use torrentleech_cli::config::{
    ConfigPaths, CredentialPrompt, CredentialResolver, EnvConfig, FileConfig, LoginOverrides,
    RawConfig, ResolveOptions, ResolvedConfig,
};
use torrentleech_cli::error::ErrorKind;

fn temp_paths(temp: &TempDir) -> ConfigPaths {
    ConfigPaths::new(
        temp.path().join("config.toml"),
        temp.path().join("cookies.json"),
        temp.path().join("downloads"),
    )
}

fn env(entries: &[(&str, &str)]) -> EnvConfig {
    EnvConfig::from_map(BTreeMap::from_iter(
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string())),
    ))
}

fn config_with_values(temp: &TempDir) -> FileConfig {
    FileConfig {
        username: Some("config-user".to_string()),
        cookie_jar: Some(temp.path().join("config-cookies.json")),
        output_dir: Some(temp.path().join("config-output")),
        default_limit: Some(25),
        password_file: Some(temp.path().join("config-password")),
    }
}

#[test]
fn resolved_settings_prefer_flags_over_env_over_config_over_defaults() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let file = RawConfig::from_file(config_with_values(&temp));
    let env = env(&[
        ("TL_CONFIG", temp.path().join("env.toml").to_str().unwrap()),
        ("TL_USERNAME", "env-user"),
        (
            "TL_COOKIE_JAR",
            temp.path().join("env-cookies.json").to_str().unwrap(),
        ),
        (
            "TL_OUTPUT_DIR",
            temp.path().join("env-output").to_str().unwrap(),
        ),
        ("TL_DEFAULT_LIMIT", "50"),
        (
            "TL_PASSWORD_FILE",
            temp.path().join("env-password").to_str().unwrap(),
        ),
    ]);
    let options = ResolveOptions {
        config_path: Some(temp.path().join("flag.toml")),
        username: Some("flag-user".to_string()),
        cookie_jar: Some(temp.path().join("flag-cookies.json")),
        output_dir: Some(temp.path().join("flag-output")),
        default_limit: Some(75),
        password_file: Some(temp.path().join("flag-password")),
        password_stdin: true,
    };

    let resolved = ResolvedConfig::resolve(file, &env, &defaults, options).unwrap();

    assert_eq!(resolved.config_path, temp.path().join("flag.toml"));
    assert_eq!(resolved.username.as_deref(), Some("flag-user"));
    assert_eq!(resolved.cookie_jar, temp.path().join("flag-cookies.json"));
    assert_eq!(resolved.output_dir, temp.path().join("flag-output"));
    assert_eq!(resolved.default_limit, Some(75));
    assert_eq!(
        resolved.password_file,
        Some(temp.path().join("flag-password"))
    );
    assert!(resolved.password_stdin);
}

#[test]
fn documented_env_vars_override_config_and_defaults_for_their_fields() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let file = RawConfig::from_file(config_with_values(&temp));
    let env_password_file = temp.path().join("env-password");
    let env_cookie_jar = temp.path().join("env-cookies.json");
    let env = env(&[
        ("TL_USERNAME", "env-user"),
        ("TL_PASSWORD_FILE", env_password_file.to_str().unwrap()),
        ("TL_COOKIE_JAR", env_cookie_jar.to_str().unwrap()),
    ]);

    let resolved =
        ResolvedConfig::resolve(file, &env, &defaults, ResolveOptions::default()).unwrap();

    assert_eq!(resolved.username.as_deref(), Some("env-user"));
    assert_eq!(
        resolved.password_file.as_deref(),
        Some(env_password_file.as_path())
    );
    assert_eq!(resolved.cookie_jar, env_cookie_jar);
}

#[test]
fn loads_toml_config_and_expands_tilde_paths() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
username = "file-user"
cookie_jar = "~/cookies.json"
output_dir = "~/downloads"
default_limit = 100
password_file = "~/secret"
"#,
    )
    .unwrap();

    let raw = RawConfig::load(&config_path, Some(temp.path())).unwrap();
    let file = raw.file.unwrap();

    assert_eq!(file.username.as_deref(), Some("file-user"));
    assert_eq!(file.cookie_jar, Some(temp.path().join("cookies.json")));
    assert_eq!(file.output_dir, Some(temp.path().join("downloads")));
    assert_eq!(file.default_limit, Some(100));
    assert_eq!(file.password_file, Some(temp.path().join("secret")));
}

#[test]
fn password_resolution_is_lazy_and_uses_login_source_precedence() {
    let temp = TempDir::new().unwrap();
    let flag_password = temp.path().join("flag-password");
    let env_password_file = temp.path().join("env-password");
    let config_password = temp.path().join("config-password");
    write_password_file(&flag_password, "flag-secret\n");
    write_password_file(&env_password_file, "env-file-secret\n");
    write_password_file(&config_password, "config-secret\n");

    let defaults = temp_paths(&temp);
    let file = RawConfig::from_file(FileConfig {
        password_file: Some(config_password),
        ..FileConfig::default()
    });
    let env = env(&[("TL_PASSWORD_FILE", env_password_file.to_str().unwrap())]);
    let resolved =
        ResolvedConfig::resolve(file, &env, &defaults, ResolveOptions::default()).unwrap();
    let login = LoginOverrides {
        password_file: Some(flag_password),
        password_stdin: true,
    };

    let mut stdin = Cursor::new(b"stdin-secret\n".to_vec());
    let mut prompt = CredentialPrompt::disabled();
    let password = CredentialResolver::new(&resolved, login)
        .read_password(&mut stdin, &mut prompt)
        .unwrap();

    assert_eq!(password, "flag-secret");
    assert!(!prompt.was_prompted());
}

#[test]
fn password_stdin_precedes_password_file_sources() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let env_password_file = temp.path().join("env-password");
    write_password_file(&env_password_file, "env-secret\n");
    let env = env(&[("TL_PASSWORD_FILE", env_password_file.to_str().unwrap())]);
    let resolved = ResolvedConfig::resolve(
        RawConfig::default(),
        &env,
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap();

    let mut stdin = Cursor::new(b"stdin-secret\n".to_vec());
    let mut prompt = CredentialPrompt::disabled();
    let password = CredentialResolver::new(
        &resolved,
        LoginOverrides {
            password_stdin: true,
            ..LoginOverrides::default()
        },
    )
    .read_password(&mut stdin, &mut prompt)
    .unwrap();

    assert_eq!(password, "stdin-secret");
}

#[cfg(unix)]
#[test]
fn group_readable_password_file_is_rejected() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let password_file = temp.path().join("password");
    fs::write(&password_file, "secret\n").unwrap();
    fs::set_permissions(&password_file, fs::Permissions::from_mode(0o640)).unwrap();
    let resolved = ResolvedConfig::resolve(
        RawConfig::default(),
        &EnvConfig::default(),
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap();

    let mut stdin = Cursor::new(Vec::new());
    let mut prompt = CredentialPrompt::disabled();
    let error = CredentialResolver::new(
        &resolved,
        LoginOverrides {
            password_file: Some(password_file),
            ..LoginOverrides::default()
        },
    )
    .read_password(&mut stdin, &mut prompt)
    .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::AuthenticationRequired);
    assert!(error.to_string().contains("group or others"));
}

#[test]
fn directory_password_file_is_rejected() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let resolved = ResolvedConfig::resolve(
        RawConfig::default(),
        &EnvConfig::default(),
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap();

    let mut stdin = Cursor::new(Vec::new());
    let mut prompt = CredentialPrompt::disabled();
    let error = CredentialResolver::new(
        &resolved,
        LoginOverrides {
            password_file: Some(temp.path().to_path_buf()),
            ..LoginOverrides::default()
        },
    )
    .read_password(&mut stdin, &mut prompt)
    .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::AuthenticationRequired);
    assert!(error.to_string().contains("regular file"));
}

#[test]
fn credential_resolution_falls_back_to_interactive_prompt() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let resolved = ResolvedConfig::resolve(
        RawConfig::default(),
        &EnvConfig::default(),
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap();

    let mut stdin = Cursor::new(Vec::new());
    let mut prompt = CredentialPrompt::provided("prompt-secret");
    let password = CredentialResolver::new(&resolved, LoginOverrides::default())
        .read_password(&mut stdin, &mut prompt)
        .unwrap();

    assert_eq!(password, "prompt-secret");
    assert!(prompt.was_prompted());
}

#[test]
fn non_tty_without_credential_source_fails_instead_of_prompting() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let resolved = ResolvedConfig::resolve(
        RawConfig::default(),
        &EnvConfig::default(),
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap();

    let mut stdin = Cursor::new(Vec::new());
    let mut prompt = CredentialPrompt::disabled();
    let error = CredentialResolver::new(&resolved, LoginOverrides::default())
        .read_password(&mut stdin, &mut prompt)
        .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::AuthenticationRequired);
    assert!(!prompt.was_prompted());
}

#[test]
fn config_show_display_never_reads_or_prints_raw_password_contents() {
    let temp = TempDir::new().unwrap();
    let password_file = temp.path().join("password-file");
    fs::write(&password_file, "super-secret").unwrap();
    let raw = RawConfig::from_file(FileConfig {
        username: Some("config-user".to_string()),
        password_file: Some(password_file.clone()),
        ..FileConfig::default()
    });

    let display = raw.to_display();
    let json = serde_json::to_string(&display).unwrap();

    assert!(json.contains("config-user"));
    assert!(json.contains(password_file.to_str().unwrap()));
    assert!(!json.contains("super-secret"));
}

#[test]
fn default_paths_are_xdg_shaped() {
    let temp = TempDir::new().unwrap();
    let paths = ConfigPaths::from_base_dirs(temp.path(), temp.path().join("state"));

    assert_eq!(paths.config_path, temp.path().join("tl/config.toml"));
    assert_eq!(paths.cookie_jar, temp.path().join("state/tl/cookies.json"));
    assert_eq!(paths.output_dir, PathBuf::from("."));
}

#[test]
fn empty_config_values_are_ignored() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let file = RawConfig::from_file(FileConfig {
        username: Some("".to_string()),
        cookie_jar: Some(PathBuf::new()),
        output_dir: Some(PathBuf::new()),
        password_file: Some(PathBuf::new()),
        ..FileConfig::default()
    });
    let resolved = ResolvedConfig::resolve(
        file,
        &EnvConfig::default(),
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap();

    assert_eq!(resolved.username, None);
    assert_eq!(resolved.cookie_jar, defaults.cookie_jar);
    assert_eq!(resolved.output_dir, defaults.output_dir);
    assert_eq!(resolved.password_file, None);
}

#[test]
fn xdg_environment_overrides_default_base_dirs() {
    let temp = TempDir::new().unwrap();
    let config_home = temp.path().join("config-home");
    let state_home = temp.path().join("state-home");
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &config_home);
        std::env::set_var("XDG_STATE_HOME", &state_home);
    }
    let paths = ConfigPaths::defaults();
    unsafe {
        std::env::remove_var("XDG_CONFIG_HOME");
        std::env::remove_var("XDG_STATE_HOME");
    }

    assert_eq!(paths.config_path, config_home.join("tl/config.toml"));
    assert_eq!(paths.cookie_jar, state_home.join("tl/cookies.json"));
}

#[test]
fn config_path_env_overrides_default_when_no_flag_is_present() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let env_path = temp.path().join("env.toml");
    let resolved = ResolvedConfig::resolve(
        RawConfig::default(),
        &env(&[("TL_CONFIG", env_path.to_str().unwrap())]),
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap();

    assert_eq!(resolved.config_path, env_path);
}

#[test]
fn malformed_toml_config_is_invalid_input() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");
    fs::write(&config_path, "username = [").unwrap();

    let error = RawConfig::load(&config_path, Some(temp.path())).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

#[test]
fn invalid_default_limit_env_is_rejected() {
    let temp = TempDir::new().unwrap();
    let defaults = temp_paths(&temp);
    let error = ResolvedConfig::resolve(
        RawConfig::default(),
        &env(&[("TL_DEFAULT_LIMIT", "many")]),
        &defaults,
        ResolveOptions::default(),
    )
    .unwrap_err();

    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

fn write_password_file(path: &Path, contents: &str) {
    fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn resolved_config_is_plain_data() {
    assert_send_sync::<ResolvedConfig>();
    assert_send_sync::<PathBuf>();
    assert_send_sync::<&Path>();
}
