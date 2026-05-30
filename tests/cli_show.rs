mod support;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use support::matchers::{method, path};
use support::{Mock, MockServer, ResponseTemplate};

fn tl() -> Command {
    Command::cargo_bin("tl").unwrap()
}

#[test]
fn show_prints_compact_details() {
    let server = MockServer::start();
    Mock::given(method("GET"))
        .and(path("/torrent/123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(detail_html()))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("show")
        .arg("123")
        .assert()
        .success()
        .stdout(predicate::str::contains("123 Example.Release"))
        .stdout(predicate::str::contains("Line one Line two"))
        .stdout(predicate::str::contains("NFO line 1"))
        .stderr("");
}

#[test]
fn show_description_only_prints_only_description() {
    let server = MockServer::start();
    Mock::given(method("GET"))
        .and(path("/torrent/123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(detail_html()))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("show")
        .arg("123")
        .arg("--description-only")
        .assert()
        .success()
        .stdout("Line one Line two\n")
        .stderr("");
}

#[test]
fn show_json_prints_stable_details() {
    let server = MockServer::start();
    Mock::given(method("GET"))
        .and(path("/torrent/123"))
        .respond_with(ResponseTemplate::new(200).set_body_string(detail_html()))
        .expect(1)
        .mount(&server);

    let output = tl()
        .arg("--base-url")
        .arg(server.uri())
        .arg("show")
        .arg("123")
        .arg("--json")
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["id"], 123);
    assert_eq!(json["title"], "Example.Release");
    assert_eq!(json["description"], "Line one Line two");
    assert_eq!(
        json["download_url"],
        format!("{}/download/123/example.release.torrent", server.uri())
    );
}

fn detail_html() -> &'static str {
    include_str!("fixtures/show/detail.html")
}
