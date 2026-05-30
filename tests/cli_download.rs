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

#[test]
fn download_id_fetches_detail_link_and_prints_saved_path_only() {
    let server = MockServer::start();
    let temp = tempdir().unwrap();
    let output_dir = temp.path().join("downloads");
    let expected_path = output_dir.join("example.release.torrent");

    Mock::given(method("GET"))
        .and(path("/torrent/123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(detail_html(123, "example.release.torrent")),
        )
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/download/123/example.release.torrent"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TORRENT_BYTES))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("download")
        .arg("123")
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--print-path")
        .assert()
        .success()
        .stdout(format!("{}\n", expected_path.display()))
        .stderr("");

    assert_eq!(fs::read(expected_path).unwrap(), TORRENT_BYTES);
}

#[test]
fn download_detail_url_uses_parsed_filename_and_prints_path_only() {
    let server = MockServer::start();
    let temp = tempdir().unwrap();
    let output_dir = temp.path().join("downloads");
    let expected_path = output_dir.join("real.name.torrent");

    Mock::given(method("GET"))
        .and(path("/torrent/123"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(detail_html(123, "real.name.torrent")),
        )
        .expect(1)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/download/123/real.name.torrent"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TORRENT_BYTES))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("download")
        .arg(format!("{}/torrent/123", server.uri()))
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--print-path")
        .assert()
        .success()
        .stdout(format!("{}\n", expected_path.display()))
        .stderr("");

    assert_eq!(fs::read(expected_path).unwrap(), TORRENT_BYTES);
}

#[test]
fn download_direct_url_skips_detail_page_and_decodes_filename() {
    let server = MockServer::start();
    let temp = tempdir().unwrap();
    let output_dir = temp.path().join("downloads");
    let expected_path = output_dir.join("direct name.torrent");

    Mock::given(method("GET"))
        .and(path("/torrent/123"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server);
    Mock::given(method("GET"))
        .and(path("/download/123/direct%20name.torrent"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TORRENT_BYTES))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("download")
        .arg(format!(
            "{}/download/123/direct%20name.torrent",
            server.uri()
        ))
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--print-path")
        .assert()
        .success()
        .stdout(format!("{}\n", expected_path.display()))
        .stderr("");

    assert_eq!(fs::read(expected_path).unwrap(), TORRENT_BYTES);
}

#[test]
fn download_conflict_writes_error_to_stderr_and_no_path_to_stdout() {
    let server = MockServer::start();
    let temp = tempdir().unwrap();
    let output_dir = temp.path().join("downloads");
    fs::create_dir_all(&output_dir).unwrap();
    let existing = output_dir.join("example.release.torrent");
    fs::write(&existing, b"kept").unwrap();

    Mock::given(method("GET"))
        .and(path("/download/123/example.release.torrent"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TORRENT_BYTES))
        .expect(1)
        .mount(&server);

    tl().arg("--base-url")
        .arg(server.uri())
        .arg("download")
        .arg(format!(
            "{}/download/123/example.release.torrent",
            server.uri()
        ))
        .arg("--output-dir")
        .arg(&output_dir)
        .arg("--print-path")
        .assert()
        .failure()
        .code(5)
        .stdout("")
        .stderr(predicate::str::contains("output file already exists"));

    assert_eq!(fs::read(existing).unwrap(), b"kept");
}

#[test]
fn download_invalid_body_leaves_no_final_file() {
    let server = MockServer::start();

    for (index, body) in [
        Vec::new(),
        b"<html>login</html>".to_vec(),
        b"not bencode".to_vec(),
        vec![b'd'; 20 * 1024 * 1024 + 1],
    ]
    .into_iter()
    .enumerate()
    {
        let temp = tempdir().unwrap();
        let output_dir = temp.path().join("downloads");
        let filename = format!("bad-{index}.torrent");

        Mock::given(method("GET"))
            .and(path(format!("/download/123/{filename}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .expect(1)
            .mount(&server);

        tl().arg("--base-url")
            .arg(server.uri())
            .arg("download")
            .arg(format!("{}/download/123/{filename}", server.uri()))
            .arg("--output-dir")
            .arg(&output_dir)
            .assert()
            .failure();

        assert!(!output_dir.join(filename).exists());
    }
}

fn detail_html(id: u64, filename: &str) -> String {
    format!(r#"<html><body><a href="/download/{id}/{filename}">download</a></body></html>"#)
}
