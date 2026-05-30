mod support;

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use support::matchers::{header, method, path};
use support::{Mock, MockServer, ResponseTemplate};
use tempfile::{TempDir, tempdir};

const LOGIN_HTML: &str = include_str!("fixtures/auth/login.html");

fn tl() -> Command {
    Command::cargo_bin("tl").unwrap()
}

fn config_path(temp: &TempDir, default_limit: Option<u32>) -> std::path::PathBuf {
    let path = temp.path().join("config.toml");
    let default_limit = default_limit
        .map(|limit| format!("default_limit = {limit}\n"))
        .unwrap_or_default();
    fs::write(
        &path,
        format!(
            "username = \"fixture-user\"\npassword_file = \"{}\"\ncookie_jar = \"{}\"\n{default_limit}",
            temp.path().join("password").display(),
            temp.path().join("cookies.json").display(),
        ),
    )
    .unwrap();
    write_password_file(&temp.path().join("password"), "fixture-password\n");
    path
}

fn write_password_file(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn browse_json(start: u64, count: usize, total: u32, page: u32) -> String {
    let torrents = (0..count)
        .map(|index| {
            let id = start + index as u64;
            format!(
                r#"{{"fid":"{id}","filename":"Ubuntu.Result.{id}.torrent","name":"Ubuntu.Result.{id}","addedTimestamp":"2026-05-29 10:15:00","categoryID":33,"size":1073741824,"completed":2,"seeders":3,"leechers":4,"numComments":1,"tags":["FREELEECH"],"download_multiplier":0}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"numFound":{total},"torrentList":[{torrents}],"page":{page},"perPage":20}}"#)
}

fn zero_json() -> &'static str {
    r#"{"numFound":0,"torrentList":[],"page":1,"perPage":20}"#
}

#[test]
fn search_fetches_pages_until_limit_and_truncates_output() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = config_path(&temp, None);

    Mock::given(method("GET"))
        .and(path(
            "/torrents/browse/list/query/ubuntu/orderby/added/order/desc/page/1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(browse_json(1, 20, 40, 1)))
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path(
            "/torrents/browse/list/query/ubuntu/orderby/added/order/desc/page/2",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(browse_json(21, 20, 40, 2)))
        .expect(1)
        .mount(&server);

    let assert = tl()
        .arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("ubuntu")
        .arg("--limit")
        .arg("25")
        .assert()
        .success()
        .stderr("");
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.lines().count(), 25);
    assert!(stdout.contains("Ubuntu.Result.1"));
    assert!(stdout.contains("Ubuntu.Result.25"));
    assert!(!stdout.contains("Ubuntu.Result.26"));
}

#[test]
fn search_uses_config_default_limit_when_flag_absent() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = config_path(&temp, Some(3));

    Mock::given(method("GET"))
        .and(path(
            "/torrents/browse/list/query/ubuntu/orderby/added/order/desc/page/1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(browse_json(1, 5, 5, 1)))
        .expect(1)
        .mount(&server);

    let assert = tl()
        .arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("ubuntu")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.lines().count(), 3);
}

#[test]
fn search_uses_builtin_default_limit_when_flag_and_config_are_absent() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = temp.path().join("missing.toml");

    Mock::given(method("GET"))
        .and(path(
            "/torrents/browse/list/query/ubuntu/orderby/added/order/desc/page/1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(browse_json(1, 12, 12, 1)))
        .expect(1)
        .mount(&server);

    let assert = tl()
        .arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("ubuntu")
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.lines().count(), 10);
}

#[test]
fn search_rejects_limits_above_one_hundred_before_requesting() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = config_path(&temp, None);

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("ubuntu")
        .arg("--limit")
        .arg("101")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("limit must be at most 100"));
}

#[test]
fn compact_search_with_zero_results_exits_success_with_empty_stdout() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = config_path(&temp, None);

    Mock::given(method("GET"))
        .and(path(
            "/torrents/browse/list/query/nope/orderby/added/order/desc/page/1",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(zero_json()))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("nope")
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn json_search_prints_search_response_shape() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = config_path(&temp, None);

    Mock::given(method("GET"))
        .and(path(
            "/torrents/browse/list/query/ubuntu/orderby/seeders/order/asc/page/2",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(browse_json(9, 1, 1, 2)))
        .expect(1)
        .mount(&server);

    let assert = tl()
        .arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("ubuntu")
        .arg("--page")
        .arg("2")
        .arg("--sort")
        .arg("seeders")
        .arg("--order")
        .arg("asc")
        .arg("--json")
        .assert()
        .success()
        .stderr("");
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["query"], "ubuntu");
    assert_eq!(value["page"], 2);
    assert_eq!(value["total"], 1);
    assert_eq!(value["results"][0]["id"], 9);
    assert_eq!(value["results"][0]["title"], "Ubuntu.Result.9");
    assert_eq!(value["results"][0]["category_id"], 33);
    assert_eq!(
        value["results"][0]["download_url"],
        format!("{}/download/9/Ubuntu.Result.9.torrent", server.uri())
    );
}

#[test]
fn expired_session_search_reuses_auth_retry() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = config_path(&temp, None);
    let browse_path = "/torrents/browse/list/query/ubuntu/orderby/added/order/desc/page/1";

    Mock::given(method("GET"))
        .and(path(browse_path))
        .respond_with(ResponseTemplate::new(302).append_header("location", "/"))
        .up_to_n_times(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path(browse_path))
        .and(header("cookie", "tluid=fixture"))
        .respond_with(ResponseTemplate::new(200).set_body_string(browse_json(1, 1, 1, 1)))
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
        .respond_with(ResponseTemplate::new(200).set_body_string("authenticated"))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("ubuntu")
        .assert()
        .success()
        .stdout(predicate::str::contains("Ubuntu.Result.1"));
}
