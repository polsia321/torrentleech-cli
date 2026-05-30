mod support;

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use support::matchers::{method, path};
use support::{Mock, MockServer, ResponseTemplate};
use tempfile::tempdir;

const TORRENT_BYTES: &[u8] = b"d4:infod3:fooi1eee";

fn tl() -> Command {
    Command::cargo_bin("tl").unwrap()
}

fn write_password_file(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

fn config_path(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let path = temp.path().join("config.toml");
    let password = temp.path().join("password");
    write_password_file(&password, "fixture-password\n");
    fs::write(
        &path,
        format!(
            "username = \"fixture-user\"\npassword_file = \"{}\"\ncookie_jar = \"{}\"\n",
            password.display(),
            temp.path().join("cookies.json").display(),
        ),
    )
    .unwrap();
    path
}

#[test]
fn invalid_input_exits_two_without_stdout() {
    tl().arg("search")
        .arg("ubuntu")
        .arg("--limit")
        .arg("101")
        .assert()
        .failure()
        .code(2)
        .stdout("")
        .stderr(predicate::str::contains("error: limit must be at most 100"));
}

#[test]
fn auth_failure_exits_three_without_stdout() {
    let temp = tempdir().unwrap();
    let missing_password = temp.path().join("missing-password");

    tl().arg("login")
        .arg("--username")
        .arg("fixture-user")
        .arg("--password-file")
        .arg(missing_password)
        .arg("--cookie-jar")
        .arg(temp.path().join("cookies.json"))
        .assert()
        .failure()
        .code(3)
        .stdout("")
        .stderr(predicate::str::contains(
            "error: failed to inspect password file",
        ));
}

#[test]
fn parse_failure_exits_four_without_stdout() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let output_dir = temp.path().join("downloads");

    Mock::given(method("GET"))
        .and(path("/download/123/not-a-torrent.torrent"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not bencode"))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("download")
        .arg(format!(
            "{}/download/123/not-a-torrent.torrent",
            server.uri()
        ))
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--print-path")
        .assert()
        .failure()
        .code(4)
        .stdout("")
        .stderr(predicate::str::contains(
            "error: torrent response was not a torrent file",
        ));
}

#[test]
fn network_failure_exits_one_without_stdout() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let config_path = config_path(&temp);

    Mock::given(method("GET"))
        .and(path(
            "/torrents/browse/list/query/ubuntu/orderby/added/order/desc/page/1",
        ))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("--config")
        .arg(config_path)
        .arg("search")
        .arg("ubuntu")
        .assert()
        .failure()
        .code(1)
        .stdout("")
        .stderr(predicate::str::contains(
            "error: request failed with status 500 Internal Server Error",
        ));
}

#[test]
fn output_conflict_exits_five_without_stdout() {
    let temp = tempdir().unwrap();
    let server = MockServer::start();
    let output_dir = temp.path().join("downloads");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(output_dir.join("exists.torrent"), b"kept").unwrap();

    Mock::given(method("GET"))
        .and(path("/download/123/exists.torrent"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TORRENT_BYTES))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("download")
        .arg(format!("{}/download/123/exists.torrent", server.uri()))
        .arg("--output-dir")
        .arg(output_dir)
        .arg("--print-path")
        .assert()
        .failure()
        .code(5)
        .stdout("")
        .stderr(predicate::str::contains(
            "error: output file already exists",
        ));
}
