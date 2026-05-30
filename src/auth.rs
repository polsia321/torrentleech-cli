use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use cookie_store::CookieStore;
use scraper::{Html, Selector};
use url::Url;

use crate::error::{ErrorKind, Result, TlError};

const LOGIN_PATH: &str = "/";
const CHALLENGE_MARKERS: &[&str] = &[
    "g-recaptcha",
    "h-captcha",
    "cf-challenge",
    "cdn-cgi/challenge-platform",
    "checking your browser",
    "enable javascript and cookies",
];

#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    username: String,
    password: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl Credentials {
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }
}

#[derive(Debug, Clone)]
pub struct LoginConfig {
    base_url: Url,
    cookie_jar: PathBuf,
    credentials: Option<Credentials>,
}

impl LoginConfig {
    #[must_use]
    pub fn new(base_url: Url, cookie_jar: PathBuf, credentials: Option<Credentials>) -> Self {
        Self {
            base_url,
            cookie_jar,
            credentials,
        }
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    #[must_use]
    pub fn cookie_jar(&self) -> &Path {
        &self.cookie_jar
    }

    #[must_use]
    pub fn credentials(&self) -> Option<&Credentials> {
        self.credentials.as_ref()
    }

    pub fn login_url(&self) -> Result<Url> {
        self.base_url.join(LOGIN_PATH).map_err(|source| {
            TlError::with_source(ErrorKind::InvalidInput, "invalid base URL", source)
        })
    }
}

#[derive(Debug, Clone)]
pub struct PersistentCookieStore {
    path: PathBuf,
    store: Arc<Mutex<CookieStore>>,
}

impl PersistentCookieStore {
    pub fn load(path: PathBuf) -> Result<Self> {
        let store = if path.exists() {
            let file = File::open(&path).map_err(|source| {
                TlError::with_source(
                    ErrorKind::InvalidInput,
                    format!("failed to read cookie jar {}", path.display()),
                    source,
                )
            })?;
            #[allow(deprecated)]
            CookieStore::load_json_all(BufReader::new(file)).map_err(|source| {
                TlError::new(
                    ErrorKind::ParseFailure,
                    format!("failed to parse cookie jar {}: {source}", path.display()),
                )
            })?
        } else {
            CookieStore::default()
        };

        Ok(Self {
            path,
            store: Arc::new(Mutex::new(store)),
        })
    }

    #[must_use]
    pub fn store(&self) -> Arc<Mutex<CookieStore>> {
        Arc::clone(&self.store)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                TlError::with_source(
                    ErrorKind::Unexpected,
                    "failed to create cookie jar directory",
                    source,
                )
            })?;
        }

        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        let mut temp_file = tempfile::NamedTempFile::new_in(parent).map_err(|source| {
            TlError::with_source(
                ErrorKind::Unexpected,
                "failed to create temporary cookie jar",
                source,
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp_file
                .as_file()
                .set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|source| {
                    TlError::with_source(
                        ErrorKind::Unexpected,
                        "failed to protect temporary cookie jar",
                        source,
                    )
                })?;
        }
        {
            let mut writer = BufWriter::new(temp_file.as_file_mut());
            let store = self.store.lock().map_err(|source| {
                TlError::new(
                    ErrorKind::Unexpected,
                    format!("failed to lock cookie store: {source}"),
                )
            })?;
            #[allow(deprecated)]
            store
                .save_incl_expired_and_nonpersistent_json(&mut writer)
                .map_err(|source| {
                    TlError::new(
                        ErrorKind::Unexpected,
                        format!("failed to serialize cookie jar: {source}"),
                    )
                })?;
            writer.flush().map_err(|source| {
                TlError::with_source(ErrorKind::Unexpected, "failed to write cookie jar", source)
            })?;
        }
        temp_file.as_file_mut().sync_all().map_err(|source| {
            TlError::with_source(ErrorKind::Unexpected, "failed to write cookie jar", source)
        })?;
        temp_file.persist(&self.path).map_err(|source| {
            TlError::with_source(
                ErrorKind::Unexpected,
                "failed to persist cookie jar",
                source.error,
            )
        })?;
        Ok(())
    }
}

pub fn login_form_action(html: &str, base_url: &Url) -> Result<Option<Url>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("form").map_err(|error| {
        TlError::new(
            ErrorKind::ParseFailure,
            format!("login form selector is invalid: {error}"),
        )
    })?;

    for form in document.select(&selector) {
        let form_html = form.html().to_ascii_lowercase();
        if !form_html.contains("password") || !form_html.contains("username") {
            continue;
        }
        let action = form.value().attr("action").unwrap_or(LOGIN_PATH);
        let url = base_url.join(action).map_err(|source| {
            TlError::with_source(ErrorKind::ParseFailure, "invalid login form action", source)
        })?;
        return Ok(Some(url));
    }

    Ok(None)
}

pub fn is_login_form(html: &str) -> bool {
    login_form_action(html, &Url::parse("https://www.torrentleech.org/").unwrap())
        .ok()
        .flatten()
        .is_some()
}

#[must_use]
pub fn is_browser_challenge(html: &str) -> bool {
    let lowered = html.to_ascii_lowercase();
    CHALLENGE_MARKERS
        .iter()
        .any(|marker| lowered.contains(marker))
}

pub fn browser_challenge_error() -> TlError {
    TlError::new(
        ErrorKind::BrowserChallengeRequired,
        "browser challenge required",
    )
}
