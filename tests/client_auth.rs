mod support;

use std::fs;

use support::matchers::{body_string_contains, header_exact, method, path};
use support::{Mock, MockServer, ResponseTemplate};
use tempfile::TempDir;
use torrentleech_cli::auth::{Credentials, LoginConfig};
use torrentleech_cli::client::TlClient;
use torrentleech_cli::error::ErrorKind;
use url::Url;

const LOGIN_HTML: &str = include_str!("fixtures/auth/login.html");
const LOGIN_FAILED_HTML: &str = include_str!("fixtures/auth/login_failed.html");
const PROTECTED_HTML: &str = include_str!("fixtures/auth/protected.html");
const CAPTCHA_HTML: &str = include_str!("fixtures/auth/captcha.html");
const CLOUDFLARE_HTML: &str = include_str!("fixtures/auth/cloudflare.html");
const BROWSER_ONLY_HTML: &str = include_str!("fixtures/auth/browser_only.html");

fn credentials() -> Credentials {
    Credentials::new("fixture-user", "fixture-password")
}

fn login_config(temp: &TempDir, server: &MockServer) -> LoginConfig {
    LoginConfig::new(
        Url::parse(&server.uri()).unwrap(),
        temp.path().join("cookies.json"),
        Some(credentials()),
    )
}

#[test]
fn login_posts_fixture_form_and_persists_private_cookies() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .up_to_n_times(1)
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .and(body_string_contains("username=fixture-user"))
        .and(body_string_contains("password=fixture-password"))
        .and(body_string_contains("token=fixture-token"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "/")
                .append_header("set-cookie", "tluid=fixture; Path=/; HttpOnly"),
        )
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("authenticated"))
        .expect(1)
        .mount(&server);

    let client = TlClient::new(login_config(&temp, &server)).unwrap();
    client.login().unwrap();

    let cookie_file = temp.path().join("cookies.json");
    let contents = fs::read_to_string(&cookie_file).unwrap();
    assert!(contents.contains("tluid"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&cookie_file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn login_with_existing_session_is_successful() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PROTECTED_HTML))
        .expect(1)
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server);

    let client = TlClient::new(login_config(&temp, &server)).unwrap();
    client.login().unwrap();
}

#[test]
fn expired_session_logs_in_once_and_retries_original_request_once() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/private"))
        .respond_with(ResponseTemplate::new(302).append_header("location", "/"))
        .up_to_n_times(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/private"))
        .and(header_exact("cookie", "tluid=fixture"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PROTECTED_HTML))
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .expect(1)
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "/user/fixture-user/profile/")
                .append_header("set-cookie", "tluid=fixture; Path=/; HttpOnly"),
        )
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/user/fixture-user/profile/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PROTECTED_HTML))
        .expect(1)
        .mount(&server);

    let client = TlClient::new(login_config(&temp, &server)).unwrap();
    let body = client.get_text("/private").unwrap();

    assert!(body.contains("private torrent list"));
}

#[test]
fn login_form_body_triggers_one_login_retry() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/private"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .up_to_n_times(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/private"))
        .and(header_exact("cookie", "tluid=fixture"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PROTECTED_HTML))
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .expect(1)
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "/user/fixture-user/profile/")
                .append_header("set-cookie", "tluid=fixture; Path=/; HttpOnly"),
        )
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/user/fixture-user/profile/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PROTECTED_HTML))
        .expect(1)
        .mount(&server);

    let client = TlClient::new(login_config(&temp, &server)).unwrap();
    let body = client.get_text("/private").unwrap();

    assert!(body.contains("private torrent list"));
}

#[test]
fn cross_origin_login_action_is_rejected_before_posting_credentials() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();
    let html = LOGIN_HTML.replace("/user/account/login/", "https://example.invalid/steal");

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(html))
        .expect(1)
        .mount(&server);

    let client = TlClient::new(login_config(&temp, &server)).unwrap();
    let error = client.login().unwrap_err();

    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

#[test]
fn login_redirecting_back_to_login_is_classified_as_failed() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .respond_with(ResponseTemplate::new(302).append_header("location", "/user/account/login/"))
        .mount(&server);

    let client = TlClient::new(login_config(&temp, &server)).unwrap();
    let error = client.login().unwrap_err();

    assert_eq!(error.kind(), ErrorKind::LoginFailed);
}

#[test]
fn credentials_debug_output_redacts_password() {
    let debug = format!("{:?}", credentials());

    assert!(debug.contains("fixture-user"));
    assert!(!debug.contains("fixture-password"));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn login_failure_is_classified() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_FAILED_HTML))
        .mount(&server);

    let client = TlClient::new(login_config(&temp, &server)).unwrap();
    let error = client.login().unwrap_err();

    assert_eq!(error.kind(), ErrorKind::LoginFailed);
}

#[test]
fn missing_credentials_on_expired_session_requires_authentication() {
    let temp = TempDir::new().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/private"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .mount(&server);

    let config = LoginConfig::new(
        Url::parse(&server.uri()).unwrap(),
        temp.path().join("cookies.json"),
        None,
    );
    let client = TlClient::new(config).unwrap();
    let error = client.get_text("/private").unwrap_err();

    assert_eq!(error.kind(), ErrorKind::AuthenticationRequired);
}

#[test]
fn browser_challenges_return_explicit_error_without_login_attempt() {
    for body in [CAPTCHA_HTML, CLOUDFLARE_HTML, BROWSER_ONLY_HTML] {
        let temp = TempDir::new().unwrap();
        let server = MockServer::start();

        Mock::given(method("GET"))
            .and(path("/private"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .expect(1)
            .mount(&server);
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server);

        let client = TlClient::new(login_config(&temp, &server)).unwrap();
        let error = client.get_text("/private").unwrap_err();

        assert_eq!(error.kind(), ErrorKind::BrowserChallengeRequired);
        assert_eq!(error.message(), "browser challenge required");
    }
}
