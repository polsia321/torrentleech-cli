use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn tl() -> Command {
    Command::cargo_bin("tl").unwrap()
}

#[test]
fn config_path_prints_active_path_even_when_missing() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("missing.toml");

    tl().env("TL_CONFIG", &config_path)
        .env("TL_DEFAULT_LIMIT", "not-a-number")
        .arg("config")
        .arg("path")
        .assert()
        .success()
        .stdout(format!("{}\n", config_path.display()))
        .stderr("");
}

#[test]
fn global_config_flag_sets_active_config_path() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("flag.toml");

    tl().env("TL_CONFIG", temp.path().join("env.toml"))
        .arg("--config")
        .arg(&config_path)
        .arg("config")
        .arg("path")
        .assert()
        .success()
        .stdout(format!("{}\n", config_path.display()))
        .stderr("");
}

#[test]
fn local_commands_ignore_invalid_base_url_env() {
    tl().env("TL_BASE_URL", "not-a-url")
        .arg("categories")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("Movies:"));
}

#[test]
fn config_init_creates_non_secret_config_and_prints_path() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("config.toml");

    tl().env("TL_CONFIG", &config_path)
        .arg("config")
        .arg("init")
        .assert()
        .success()
        .stdout(format!("{}\n", config_path.display()))
        .stderr("");

    let contents = std::fs::read_to_string(&config_path).unwrap();
    assert!(!contents.contains("username"));
    assert!(!contents.contains("cookie_jar"));
    assert!(contents.contains("output_dir"));
    assert!(contents.contains("default_limit"));
    assert!(!contents.contains("password"));
}

#[test]
fn config_init_fails_without_overwriting_existing_config() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    std::fs::write(&config_path, "username = \"kept\"\n").unwrap();

    tl().env("TL_CONFIG", &config_path)
        .arg("config")
        .arg("init")
        .assert()
        .failure()
        .code(5)
        .stderr(predicate::str::contains("already exists"));

    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        "username = \"kept\"\n"
    );
}

#[test]
fn config_show_prints_file_values_without_password_contents_or_env_overrides() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    let password_path = temp.path().join("password");
    std::fs::write(&password_path, "super-secret\n").unwrap();
    std::fs::write(
        &config_path,
        format!(
            "username = \"file-user\"\ndefault_limit = 25\npassword_file = \"{}\"\n",
            password_path.display()
        ),
    )
    .unwrap();

    tl().env("TL_CONFIG", &config_path)
        .env("TL_USERNAME", "env-user")
        .arg("config")
        .arg("show")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("file-user"))
        .stdout(predicate::str::contains(
            password_path.to_string_lossy().as_ref(),
        ))
        .stdout(predicate::str::contains("super-secret").not())
        .stdout(predicate::str::contains("env-user").not())
        .stdout(predicate::str::contains("env-secret").not());
}

#[test]
fn categories_json_is_stable_and_structured() {
    tl().arg("categories")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "{\n  \"groups\": [\n    {\n      \"name\": \"Movies\"",
        ))
        .stdout(predicate::str::contains(
            "\"categories\": [\n        {\n          \"id\": 8,\n          \"name\": \"Cam\",\n          \"aliases\": []\n        }",
        ))
        .stdout(predicate::str::contains("\"name\": \"Foreign\""))
        .stdout(predicate::str::contains("\"id\": 44"));
}

#[test]
fn categories_text_is_stable_and_compact() {
    tl().arg("categories")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "Movies: 8 Cam, 9 TS/TC, 11 DVDRip/DVDScreener",
        ))
        .stdout(predicate::str::contains(
            "\nApps: 23 PC-ISO, 24 Mac, 25 Mobile, 33 0-day",
        ))
        .stdout(predicate::str::contains(
            "\nForeign: 36 Movies foreign, 44 TV Series foreign\n",
        ))
        .stderr("");
}
