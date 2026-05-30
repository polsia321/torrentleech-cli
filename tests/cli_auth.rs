mod support;

use assert_cmd::Command;
use predicates::prelude::*;
use support::matchers::{body_string_contains, header, method, path};
use support::{Mock, MockServer, ResponseTemplate};
use tempfile::tempdir;

const LOGIN_HTML: &str = include_str!("fixtures/auth/login.html");

fn write_password_file(path: &std::path::Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn tl() -> Command {
    Command::cargo_bin("tl").unwrap()
}

#[test]
fn login_uses_env_credentials_and_saves_cookie_without_printing_secrets() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let cookie_jar = temp.path().join("cookies.json");

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .expect(1)
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .and(body_string_contains("username=fixture-user"))
        .and(body_string_contains("password=fixture-password"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "/user/fixture-user/profile/")
                .append_header("set-cookie", "tluid=fixture; Path=/; HttpOnly"),
        )
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/user/fixture-user/profile/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("authenticated"))
        .expect(1)
        .mount(&server);

    let password_file = temp.path().join("password");
    write_password_file(&password_file, "fixture-password\n");

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("login")
        .arg("--cookie-jar")
        .arg(&cookie_jar)
        .env("TL_USERNAME", "fixture-user")
        .env("TL_PASSWORD_FILE", &password_file)
        .assert()
        .success()
        .stdout(format!("saved session to {}\n", cookie_jar.display()))
        .stderr(predicate::str::contains("fixture-password").not());

    let cookies = std::fs::read_to_string(cookie_jar).unwrap();
    assert!(cookies.contains("tluid"));
    assert!(!cookies.contains("fixture-password"));
}

#[test]
fn login_uses_config_username_and_password_stdin() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = temp.path().join("config.toml");
    let cookie_jar = temp.path().join("cookies.json");
    std::fs::write(
        &config_path,
        format!(
            "username = \"fixture-user\"\ncookie_jar = \"{}\"\n",
            cookie_jar.display()
        ),
    )
    .unwrap();

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .and(body_string_contains("username=fixture-user"))
        .and(body_string_contains("password=stdin-password"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "/user/fixture-user/profile/")
                .append_header("set-cookie", "tluid=fixture; Path=/; HttpOnly"),
        )
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/user/fixture-user/profile/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("authenticated"))
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("login")
        .arg("--password-stdin")
        .write_stdin("stdin-password\n")
        .assert()
        .success()
        .stdout(format!("saved session to {}\n", cookie_jar.display()))
        .stderr("");

    assert!(
        std::fs::read_to_string(cookie_jar)
            .unwrap()
            .contains("tluid")
    );
}

#[test]
fn whoami_prints_username_from_authenticated_navigation() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let cookie_jar = temp.path().join("cookies.json");

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .up_to_n_times(1)
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "/user/fixture-user/profile/")
                .append_header("set-cookie", "tluid=fixture; Path=/; HttpOnly"),
        )
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/user/fixture-user/profile/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("authenticated"))
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/"))
        .and(header("cookie", "tluid=fixture"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<!doctype html>
<html>
  <body>
    <header>
      <nav>
        <a href="/user/fixture-user/profile/">fixture-user</a>
        <a href="/user/account/logout/">Logout</a>
      </nav>
    </header>
  </body>
</html>"#,
        ))
        .expect(1)
        .mount(&server);

    let password_file = temp.path().join("password");
    write_password_file(&password_file, "fixture-password\n");

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("login")
        .arg("--cookie-jar")
        .arg(&cookie_jar)
        .env("TL_USERNAME", "fixture-user")
        .env("TL_PASSWORD_FILE", &password_file)
        .assert()
        .success();

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("whoami")
        .arg("--cookie-jar")
        .arg(cookie_jar)
        .env("TL_USERNAME", "fixture-user")
        .env("TL_PASSWORD_FILE", &password_file)
        .assert()
        .success()
        .stdout("fixture-user\n")
        .stderr("");
}

#[test]
fn login_failure_exits_with_auth_code() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();

    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(LOGIN_HTML))
        .mount(&server);
    Mock::given(method("POST"))
        .and(path("/user/account/login/"))
        .respond_with(ResponseTemplate::new(302).append_header("location", "/user/account/login/"))
        .mount(&server);

    let password_file = temp.path().join("password");
    write_password_file(&password_file, "fixture-password\n");

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("login")
        .arg("--cookie-jar")
        .arg(temp.path().join("cookies.json"))
        .env("TL_USERNAME", "fixture-user")
        .env("TL_PASSWORD_FILE", &password_file)
        .assert()
        .failure()
        .code(3)
        .stdout("")
        .stderr(predicate::str::contains("login failed"));
}

#[test]
fn logout_removes_cookie_jar_and_is_idempotent() {
    let temp = tempdir().unwrap();
    let cookie_jar = temp.path().join("cookies.json");
    std::fs::write(&cookie_jar, "session").unwrap();

    tl().arg("logout")
        .arg("--cookie-jar")
        .arg(&cookie_jar)
        .assert()
        .success()
        .stdout("")
        .stderr("");
    assert!(!cookie_jar.exists());

    tl().arg("logout")
        .arg("--cookie-jar")
        .arg(cookie_jar)
        .assert()
        .success()
        .stdout("")
        .stderr("");
}
