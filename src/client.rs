use std::io::Read;

use cookie_store::CookieStore;
use ureq::http::{HeaderMap, Method, Response, header};
use ureq::middleware::{Middleware, MiddlewareNext};
use url::Url;

use crate::auth::{
    LoginConfig, PersistentCookieStore, browser_challenge_error, is_browser_challenge,
    is_login_form, login_form_action,
};
use crate::error::{ErrorKind, Result, TlError};

#[derive(Debug, Clone)]
pub struct TlClient {
    http: ureq::Agent,
    config: LoginConfig,
    cookies: PersistentCookieStore,
}

impl TlClient {
    pub fn new(config: LoginConfig) -> Result<Self> {
        let cookies = PersistentCookieStore::load(config.cookie_jar().to_path_buf())?;
        let http = ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .user_agent("torrentleech-cli/0.1")
            .proxy(None)
            .middleware(CookieMiddleware {
                store: cookies.store(),
            })
            .build()
            .new_agent();

        Ok(Self {
            http,
            config,
            cookies,
        })
    }

    pub fn login(&self) -> Result<()> {
        let credentials = self.config.credentials().ok_or_else(|| {
            TlError::new(
                ErrorKind::AuthenticationRequired,
                "credentials required to authenticate",
            )
        })?;
        let login_url = self.config.login_url()?;
        let mut login_page = self
            .http
            .get(login_url.as_str())
            .call()
            .map_err(network_error)?;
        let login_page_url = response_url(&login_page).ok_or_else(|| {
            TlError::new(ErrorKind::NetworkFailure, "HTTP response URL was missing")
        })?;
        let login_html = login_page
            .body_mut()
            .read_to_string()
            .map_err(network_error)?;

        if is_browser_challenge(&login_html) {
            return Err(browser_challenge_error());
        }

        let Some(action) = login_form_action(&login_html, &login_page_url)? else {
            if looks_authenticated(&login_html) {
                self.cookies.save()?;
                return Ok(());
            }
            return Err(TlError::new(
                ErrorKind::ParseFailure,
                "login form was not found",
            ));
        };
        ensure_same_origin(&action, self.config.base_url())?;
        let mut form = hidden_inputs(&login_html);
        form.push(("username".to_string(), credentials.username().to_string()));
        form.push(("password".to_string(), credentials.password().to_string()));
        let response = self
            .http
            .post(action.as_str())
            .send_form(
                form.iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            )
            .map_err(network_error)?;

        self.finish_login_response(response)?;
        self.cookies.save()?;
        Ok(())
    }

    pub fn get_text(&self, path_or_url: &str) -> Result<String> {
        let url = self.resolve_url(path_or_url)?;
        let response = self.send(Method::GET, url.clone())?;
        match self.response_text(response)? {
            ResponseBody::Authenticated(body) => Ok(body),
            ResponseBody::ExpiredSession => {
                self.login()?;
                let response = self.send(Method::GET, url)?;
                self.response_text(response)?.into_authenticated()
            }
        }
    }

    pub fn get_bytes_limited(&self, path_or_url: &str, limit: usize) -> Result<Vec<u8>> {
        let url = self.resolve_url(path_or_url)?;
        let response = self.send(Method::GET, url.clone())?;
        match self.response_bytes(response, limit)? {
            BytesResponseBody::Authenticated(body) => Ok(body),
            BytesResponseBody::ExpiredSession => {
                self.login()?;
                let response = self.send(Method::GET, url)?;
                self.response_bytes(response, limit)?.into_authenticated()
            }
        }
    }

    fn finish_login_response(&self, response: Response<ureq::Body>) -> Result<()> {
        if let Some(location) = redirect_location(&response) {
            ensure_same_origin(&location, self.config.base_url())?;
            if location.path() == "/user/account/login/" {
                return Err(TlError::new(ErrorKind::LoginFailed, "login failed"));
            }
            let response = self.send(Method::GET, location)?;
            return match self.response_text(response)? {
                ResponseBody::Authenticated(_) => Ok(()),
                ResponseBody::ExpiredSession => {
                    Err(TlError::new(ErrorKind::LoginFailed, "login failed"))
                }
            };
        }

        let status = response.status();
        let body = read_text(response)?;
        if is_browser_challenge(&body) {
            return Err(browser_challenge_error());
        }
        if is_login_form(&body)
            || status.is_redirection()
            || status.is_client_error()
            || status.is_server_error()
        {
            return Err(TlError::new(ErrorKind::LoginFailed, "login failed"));
        }
        Ok(())
    }

    pub fn post_form_text(&self, path_or_url: &str, form: &[(&str, &str)]) -> Result<String> {
        let url = self.resolve_url(path_or_url)?;
        let response = self
            .http
            .post(url.as_str())
            .send_form(form.iter().copied())
            .map_err(network_error)?;
        match self.response_text(response)? {
            ResponseBody::Authenticated(body) => Ok(body),
            ResponseBody::ExpiredSession => {
                self.login()?;
                let response = self
                    .http
                    .post(url.as_str())
                    .send_form(form.iter().copied())
                    .map_err(network_error)?;
                self.response_text(response)?.into_authenticated()
            }
        }
    }

    fn resolve_url(&self, path_or_url: &str) -> Result<Url> {
        let url = self.config.base_url().join(path_or_url).map_err(|source| {
            TlError::with_source(ErrorKind::InvalidInput, "invalid request URL", source)
        })?;
        ensure_same_origin(&url, self.config.base_url())?;
        Ok(url)
    }

    fn send(&self, method: Method, url: Url) -> Result<Response<ureq::Body>> {
        match method {
            Method::GET => self.http.get(url.as_str()).call().map_err(network_error),
            _ => Err(TlError::new(
                ErrorKind::Unexpected,
                format!("unsupported HTTP method {method}"),
            )),
        }
    }

    fn response_text(&self, response: Response<ureq::Body>) -> Result<ResponseBody> {
        if self.is_login_redirect(&response) {
            return Ok(ResponseBody::ExpiredSession);
        }

        let status = response.status();
        let body = read_text(response)?;
        if is_browser_challenge(&body) {
            return Err(browser_challenge_error());
        }
        if is_login_form(&body) {
            return Ok(ResponseBody::ExpiredSession);
        }
        if !status.is_success() {
            return Err(TlError::new(
                ErrorKind::NetworkFailure,
                format!("request failed with status {status}"),
            ));
        }
        Ok(ResponseBody::Authenticated(body))
    }

    fn response_bytes(
        &self,
        mut response: Response<ureq::Body>,
        limit: usize,
    ) -> Result<BytesResponseBody> {
        if self.is_login_redirect(&response) {
            return Ok(BytesResponseBody::ExpiredSession);
        }

        let status = response.status();
        let mut body = Vec::new();
        let mut reader = response.body_mut().as_reader().take((limit + 1) as u64);
        reader.read_to_end(&mut body).map_err(network_error)?;
        if body.len() > limit {
            return Err(TlError::new(
                ErrorKind::ParseFailure,
                "torrent response was too large",
            ));
        }

        if let Ok(text) = std::str::from_utf8(&body) {
            if is_browser_challenge(text) {
                return Err(browser_challenge_error());
            }
            if is_login_form(text) {
                return Ok(BytesResponseBody::ExpiredSession);
            }
        }
        if !status.is_success() {
            return Err(TlError::new(
                ErrorKind::NetworkFailure,
                format!("request failed with status {status}"),
            ));
        }

        Ok(BytesResponseBody::Authenticated(body))
    }

    fn is_login_redirect(&self, response: &Response<ureq::Body>) -> bool {
        if !response.status().is_redirection() {
            return false;
        }
        redirect_location(response)
            .map(|location| location.path() == "/" || location.path() == "/user/account/login/")
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ResponseBody {
    Authenticated(String),
    ExpiredSession,
}

impl ResponseBody {
    fn into_authenticated(self) -> Result<String> {
        match self {
            Self::Authenticated(body) => Ok(body),
            Self::ExpiredSession => Err(TlError::new(
                ErrorKind::AuthenticationRequired,
                "session expired after retry",
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BytesResponseBody {
    Authenticated(Vec<u8>),
    ExpiredSession,
}

impl BytesResponseBody {
    fn into_authenticated(self) -> Result<Vec<u8>> {
        match self {
            Self::Authenticated(body) => Ok(body),
            Self::ExpiredSession => Err(TlError::new(
                ErrorKind::AuthenticationRequired,
                "session expired after retry",
            )),
        }
    }
}

fn read_text(mut response: Response<ureq::Body>) -> Result<String> {
    response.body_mut().read_to_string().map_err(network_error)
}

fn redirect_location(response: &Response<ureq::Body>) -> Option<Url> {
    let location = response.headers().get(header::LOCATION)?.to_str().ok()?;
    response_url(response)?.join(location).ok()
}

fn response_url(response: &Response<ureq::Body>) -> Option<Url> {
    use ureq::ResponseExt;

    response.get_uri().to_string().parse().ok()
}

fn ensure_same_origin(url: &Url, base_url: &Url) -> Result<()> {
    if url.scheme() != base_url.scheme()
        || url.host_str() != base_url.host_str()
        || url.port_or_known_default() != base_url.port_or_known_default()
    {
        return Err(TlError::new(
            ErrorKind::InvalidInput,
            "URL host must match the configured TorrentLeech host",
        ));
    }
    Ok(())
}

fn network_error(source: impl Into<anyhow::Error>) -> TlError {
    TlError::with_source(ErrorKind::NetworkFailure, "HTTP request failed", source)
}

fn looks_authenticated(html: &str) -> bool {
    let lowered = html.to_ascii_lowercase();
    lowered.contains("/user/account/logout")
        || lowered.contains("logout")
        || lowered.contains("nav-tl-search")
        || lowered.contains("/torrents/browse")
}

fn hidden_inputs(html: &str) -> Vec<(String, String)> {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("input[type=hidden][name]")
        .expect("hidden input selector is valid");
    document
        .select(&selector)
        .filter_map(|input| {
            let name = input.value().attr("name")?;
            let value = input.value().attr("value").unwrap_or_default();
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

#[derive(Debug)]
struct CookieMiddleware {
    store: std::sync::Arc<std::sync::Mutex<CookieStore>>,
}

impl Middleware for CookieMiddleware {
    fn handle(
        &self,
        mut request: ureq::http::Request<ureq::SendBody>,
        next: MiddlewareNext,
    ) -> std::result::Result<Response<ureq::Body>, ureq::Error> {
        let url = request.uri().to_string().parse::<Url>().map_err(|error| {
            ureq::Error::BadUri(format!("failed to parse request URL for cookies: {error}"))
        })?;
        if !request.headers().contains_key(header::COOKIE) {
            let cookie_header = {
                let store = self.store.lock().map_err(|error| {
                    ureq::Error::Other(Box::new(CookieLockError(error.to_string())))
                })?;
                request_cookies(&store, &url)
            };
            if !cookie_header.is_empty() {
                request.headers_mut().insert(
                    header::COOKIE,
                    ureq::http::HeaderValue::from_str(&cookie_header)
                        .map_err(|error| ureq::Error::Http(error.into()))?,
                );
            }
        }

        let response = next.handle(request)?;
        store_response_cookies(&self.store, &url, response.headers())?;
        Ok(response)
    }
}

fn request_cookies(store: &CookieStore, url: &Url) -> String {
    store
        .get_request_values(url)
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn store_response_cookies(
    store: &std::sync::Mutex<CookieStore>,
    url: &Url,
    headers: &HeaderMap,
) -> std::result::Result<(), ureq::Error> {
    let cookies = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| cookie_store::RawCookie::parse(value.to_string()).ok())
        .map(cookie_store::RawCookie::into_owned)
        .collect::<Vec<_>>();
    if cookies.is_empty() {
        return Ok(());
    }
    store
        .lock()
        .map_err(|error| ureq::Error::Other(Box::new(CookieLockError(error.to_string()))))?
        .store_response_cookies(cookies.into_iter(), url);
    Ok(())
}

#[derive(Debug)]
struct CookieLockError(String);

impl std::fmt::Display for CookieLockError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CookieLockError {}
