use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, IsTerminal};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ErrorKind, Result, TlError};

const APP_DIR: &str = "tl";
const DEFAULT_CONFIG_FILE: &str = "config.toml";
const DEFAULT_COOKIE_FILE: &str = "cookies.json";
const TL_CONFIG: &str = "TL_CONFIG";
const TL_USERNAME: &str = "TL_USERNAME";
const TL_PASSWORD_FILE: &str = "TL_PASSWORD_FILE";
const TL_COOKIE_JAR: &str = "TL_COOKIE_JAR";
const TL_OUTPUT_DIR: &str = "TL_OUTPUT_DIR";
const TL_DEFAULT_LIMIT: &str = "TL_DEFAULT_LIMIT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPaths {
    pub config_path: PathBuf,
    pub cookie_jar: PathBuf,
    pub output_dir: PathBuf,
}

impl ConfigPaths {
    #[must_use]
    pub const fn new(config_path: PathBuf, cookie_jar: PathBuf, output_dir: PathBuf) -> Self {
        Self {
            config_path,
            cookie_jar,
            output_dir,
        }
    }

    #[must_use]
    pub fn from_base_dirs(config_dir: impl AsRef<Path>, state_dir: impl AsRef<Path>) -> Self {
        Self {
            config_path: config_dir.as_ref().join(APP_DIR).join(DEFAULT_CONFIG_FILE),
            cookie_jar: state_dir.as_ref().join(APP_DIR).join(DEFAULT_COOKIE_FILE),
            output_dir: PathBuf::from("."),
        }
    }

    #[must_use]
    pub fn defaults() -> Self {
        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".config"));
        let state_dir = std::env::var_os("XDG_STATE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".local/state"));
        Self::from_base_dirs(config_dir, state_dir)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvConfig {
    values: BTreeMap<String, String>,
}

impl EnvConfig {
    #[must_use]
    pub fn current() -> Self {
        Self::from_map(std::env::vars().collect())
    }

    #[must_use]
    pub const fn from_map(values: BTreeMap<String, String>) -> Self {
        Self { values }
    }

    fn string(&self, key: &str) -> Option<String> {
        self.values
            .get(key)
            .filter(|value| !value.is_empty())
            .cloned()
    }

    fn path(&self, key: &str) -> Option<PathBuf> {
        self.string(key).map(PathBuf::from)
    }

    #[must_use]
    pub fn config_path(&self) -> Option<PathBuf> {
        self.path(TL_CONFIG)
    }

    fn default_limit(&self) -> Result<Option<u32>> {
        self.string(TL_DEFAULT_LIMIT)
            .map(|value| {
                value.parse::<u32>().map_err(|source| {
                    TlError::with_source(
                        ErrorKind::InvalidInput,
                        format!("{TL_DEFAULT_LIMIT} must be an unsigned integer"),
                        source,
                    )
                })
            })
            .transpose()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawConfig {
    pub file: Option<FileConfig>,
}

impl RawConfig {
    #[must_use]
    pub const fn from_file(file: FileConfig) -> Self {
        Self { file: Some(file) }
    }

    pub fn load(path: impl AsRef<Path>, home_dir: Option<&Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| {
            TlError::with_source(
                ErrorKind::InvalidInput,
                format!("failed to read config file {}", path.display()),
                source,
            )
        })?;
        let mut file = toml::from_str::<FileConfig>(&contents).map_err(|source| {
            TlError::with_source(
                ErrorKind::InvalidInput,
                format!("failed to parse config file {}", path.display()),
                source,
            )
        })?;
        file.expand_paths(home_dir);
        Ok(Self::from_file(file))
    }

    #[must_use]
    pub fn to_display(&self) -> ConfigDisplay {
        ConfigDisplay {
            file: self.file.as_ref().map(FileConfigDisplay::from),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub username: Option<String>,
    pub cookie_jar: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub default_limit: Option<u32>,
    pub password_file: Option<PathBuf>,
}

impl FileConfig {
    fn expand_paths(&mut self, home_dir: Option<&Path>) {
        self.username = self.username.take().and_then(non_empty_string);
        self.cookie_jar = self
            .cookie_jar
            .take()
            .and_then(non_empty_path)
            .map(|path| expand_home(path, home_dir));
        self.output_dir = self
            .output_dir
            .take()
            .and_then(non_empty_path)
            .map(|path| expand_home(path, home_dir));
        self.password_file = self
            .password_file
            .take()
            .and_then(non_empty_path)
            .map(|path| expand_home(path, home_dir));
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveOptions {
    pub config_path: Option<PathBuf>,
    pub username: Option<String>,
    pub cookie_jar: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub default_limit: Option<u32>,
    pub password_file: Option<PathBuf>,
    pub password_stdin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub config_path: PathBuf,
    pub username: Option<String>,
    pub cookie_jar: PathBuf,
    pub output_dir: PathBuf,
    pub default_limit: Option<u32>,
    pub password_file: Option<PathBuf>,
    pub password_stdin: bool,
    env_password_file: Option<PathBuf>,
    config_password_file: Option<PathBuf>,
}

impl ResolvedConfig {
    pub fn resolve(
        raw: RawConfig,
        env: &EnvConfig,
        defaults: &ConfigPaths,
        options: ResolveOptions,
    ) -> Result<Self> {
        let file = raw.file.unwrap_or_default();
        let env_default_limit = env.default_limit()?;
        let env_password_file = env.path(TL_PASSWORD_FILE);
        let config_password_file = file.password_file.clone();

        Ok(Self {
            config_path: first_path(
                options.config_path,
                env.path(TL_CONFIG),
                Some(defaults.config_path.clone()),
            )
            .expect("default config path is present"),
            username: first_string(options.username, env.string(TL_USERNAME), file.username),
            cookie_jar: first_path(options.cookie_jar, env.path(TL_COOKIE_JAR), file.cookie_jar)
                .unwrap_or_else(|| defaults.cookie_jar.clone()),
            output_dir: first_path(options.output_dir, env.path(TL_OUTPUT_DIR), file.output_dir)
                .unwrap_or_else(|| defaults.output_dir.clone()),
            default_limit: options
                .default_limit
                .or(env_default_limit)
                .or(file.default_limit),
            password_file: first_path(
                options.password_file.clone(),
                env_password_file.clone(),
                file.password_file,
            ),
            password_stdin: options.password_stdin,
            env_password_file,
            config_password_file,
        })
    }

    #[must_use]
    pub fn password_sources(&self, login: LoginOverrides) -> PasswordSources {
        PasswordSources {
            flag_password_file: login.password_file,
            password_stdin: login.password_stdin,
            env_password_file: self.env_password_file.clone(),
            config_password_file: self.config_password_file.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoginOverrides {
    pub password_file: Option<PathBuf>,
    pub password_stdin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordSources {
    flag_password_file: Option<PathBuf>,
    password_stdin: bool,
    env_password_file: Option<PathBuf>,
    config_password_file: Option<PathBuf>,
}

pub struct CredentialResolver<'a> {
    resolved: &'a ResolvedConfig,
    login: LoginOverrides,
}

impl<'a> CredentialResolver<'a> {
    #[must_use]
    pub const fn new(resolved: &'a ResolvedConfig, login: LoginOverrides) -> Self {
        Self { resolved, login }
    }

    pub fn read_password(
        self,
        stdin: &mut impl BufRead,
        prompt: &mut CredentialPrompt,
    ) -> Result<String> {
        let sources = self.resolved.password_sources(self.login);
        if let Some(path) = sources.flag_password_file {
            return read_password_file(&path);
        }
        if sources.password_stdin {
            return read_password_stdin(stdin);
        }
        if let Some(path) = sources.env_password_file {
            return read_password_file(&path);
        }
        if let Some(path) = sources.config_password_file {
            return read_password_file(&path);
        }
        prompt.read_password()
    }
}

#[derive(Debug, Clone)]
pub enum CredentialPrompt {
    Tty { message: String, prompted: bool },
    Provided { password: String, prompted: bool },
    Disabled,
}

impl CredentialPrompt {
    #[must_use]
    pub fn tty() -> Self {
        Self::Tty {
            message: "Password: ".to_string(),
            prompted: false,
        }
    }

    #[must_use]
    pub fn provided(password: impl Into<String>) -> Self {
        Self::Provided {
            password: password.into(),
            prompted: false,
        }
    }

    #[must_use]
    pub const fn disabled() -> Self {
        Self::Disabled
    }

    #[must_use]
    pub const fn was_prompted(&self) -> bool {
        match self {
            Self::Tty { prompted, .. } | Self::Provided { prompted, .. } => *prompted,
            Self::Disabled => false,
        }
    }

    fn read_password(&mut self) -> Result<String> {
        match self {
            Self::Tty { message, prompted } => {
                if !io::stdin().is_terminal() {
                    return Err(missing_password_source());
                }
                *prompted = true;
                rpassword::prompt_password(message).map_err(|source| {
                    TlError::with_source(
                        ErrorKind::AuthenticationRequired,
                        "failed to read password from TTY",
                        source,
                    )
                })
            }
            Self::Provided { password, prompted } => {
                *prompted = true;
                Ok(password.clone())
            }
            Self::Disabled => Err(missing_password_source()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigDisplay {
    pub file: Option<FileConfigDisplay>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileConfigDisplay {
    pub username: Option<String>,
    pub cookie_jar: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub default_limit: Option<u32>,
    pub password_file: Option<PathBuf>,
}

impl From<&FileConfig> for FileConfigDisplay {
    fn from(value: &FileConfig) -> Self {
        Self {
            username: value.username.clone(),
            cookie_jar: value.cookie_jar.clone(),
            output_dir: value.output_dir.clone(),
            default_limit: value.default_limit,
            password_file: value.password_file.clone(),
        }
    }
}

fn first_string(
    flag: Option<String>,
    env: Option<String>,
    config: Option<String>,
) -> Option<String> {
    flag.and_then(non_empty_string)
        .or_else(|| env.and_then(non_empty_string))
        .or_else(|| config.and_then(non_empty_string))
}

fn first_path(
    flag: Option<PathBuf>,
    env: Option<PathBuf>,
    config: Option<PathBuf>,
) -> Option<PathBuf> {
    flag.and_then(non_empty_path)
        .or_else(|| env.and_then(non_empty_path))
        .or_else(|| config.and_then(non_empty_path))
}

fn non_empty_string(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn non_empty_path(path: PathBuf) -> Option<PathBuf> {
    (!path.as_os_str().is_empty()).then_some(path)
}

fn expand_home(path: PathBuf, home_dir: Option<&Path>) -> PathBuf {
    let Some(home_dir) = home_dir else {
        return path;
    };
    let Ok(stripped) = path.strip_prefix("~") else {
        return path;
    };
    if stripped.as_os_str().is_empty() {
        home_dir.to_path_buf()
    } else {
        home_dir.join(stripped)
    }
}

fn read_password_file(path: &Path) -> Result<String> {
    validate_password_file(path)?;
    fs::read_to_string(path)
        .map(|password| password.trim_end_matches(['\r', '\n']).to_string())
        .map_err(|source| {
            TlError::with_source(
                ErrorKind::AuthenticationRequired,
                format!("failed to read password file {}", path.display()),
                source,
            )
        })
}

#[cfg(unix)]
fn validate_password_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::symlink_metadata(path).map_err(|source| {
        TlError::with_source(
            ErrorKind::AuthenticationRequired,
            format!("failed to inspect password file {}", path.display()),
            source,
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(TlError::new(
            ErrorKind::AuthenticationRequired,
            format!("password file must be a regular file: {}", path.display()),
        ));
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(TlError::new(
            ErrorKind::AuthenticationRequired,
            format!(
                "password file must not be readable by group or others: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_password_file(_path: &Path) -> Result<()> {
    Ok(())
}

fn read_password_stdin(stdin: &mut impl BufRead) -> Result<String> {
    let mut password = String::new();
    stdin.read_line(&mut password).map_err(|source| {
        TlError::with_source(
            ErrorKind::AuthenticationRequired,
            "failed to read password from stdin",
            source,
        )
    })?;
    Ok(password.trim_end_matches(['\r', '\n']).to_string())
}

fn missing_password_source() -> TlError {
    TlError::new(
        ErrorKind::AuthenticationRequired,
        "password required but no credential source is available",
    )
}
