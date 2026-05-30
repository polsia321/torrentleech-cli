use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;
use torrentleech_cli::download::{
    ConflictPolicy, OutputRequest, TargetKind, parse_detail_download_link, persist_torrent,
    resolve_target, sanitize_torrent_filename, validate_torrent_response,
};
use torrentleech_cli::error::ErrorKind;
use url::Url;

fn base_url() -> Url {
    Url::parse("https://www.torrentleech.org/").unwrap()
}

#[test]
fn resolves_id_detail_url_and_direct_download_url() {
    let id = resolve_target("12345", &base_url()).unwrap();
    assert_eq!(id.id, 12345);
    assert_eq!(id.filename_hint, None);
    assert_eq!(
        id.url,
        Url::parse("https://www.torrentleech.org/torrent/12345").unwrap()
    );
    assert_eq!(id.kind, TargetKind::Detail);

    let detail = resolve_target("https://www.torrentleech.org/torrent/67890", &base_url()).unwrap();
    assert_eq!(detail.id, 67890);
    assert_eq!(detail.filename_hint, None);
    assert_eq!(detail.kind, TargetKind::Detail);

    let direct = resolve_target(
        "https://www.torrentleech.org/download/42/example.release.torrent",
        &base_url(),
    )
    .unwrap();
    assert_eq!(direct.id, 42);
    assert_eq!(
        direct.filename_hint.as_deref(),
        Some("example.release.torrent")
    );
    assert_eq!(direct.kind, TargetKind::DirectDownload);

    let with_plus = resolve_target(
        "https://www.torrentleech.org/download/42/A+B.torrent",
        &base_url(),
    )
    .unwrap();
    assert_eq!(with_plus.filename_hint.as_deref(), Some("A+B.torrent"));
}

#[test]
fn rejects_non_torrentleech_urls_before_http() {
    let error =
        resolve_target("https://evil.example/download/42/a.torrent", &base_url()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

#[test]
fn parses_detail_page_download_link() {
    let html = include_str!("fixtures/download/detail.html");
    let link = parse_detail_download_link(html, 12345, &base_url()).unwrap();

    assert_eq!(
        link,
        Url::parse("https://www.torrentleech.org/download/12345/example.release.torrent").unwrap()
    );

    let html_with_external_anchor =
        r#"<a href="https://evil.example/x">x</a><a href="/download/12345/file.torrent">ok</a>"#;
    let link = parse_detail_download_link(html_with_external_anchor, 12345, &base_url()).unwrap();
    assert_eq!(
        link,
        Url::parse("https://www.torrentleech.org/download/12345/file.torrent").unwrap()
    );
}

#[test]
fn sanitizes_torrent_filenames() {
    assert_eq!(
        sanitize_torrent_filename("../bad\\name\0with\ttabs\nand lines"),
        "_bad_name_with_tabs_and lines.torrent"
    );
    assert_eq!(sanitize_torrent_filename("...hidden"), "hidden.torrent");
    assert_eq!(sanitize_torrent_filename("a\rb"), "a_b.torrent");
    assert_eq!(sanitize_torrent_filename("release"), "release.torrent");
    assert_eq!(sanitize_torrent_filename(""), "download.torrent");

    let oversized = format!("{}.torrent", "a".repeat(400));
    let sanitized = sanitize_torrent_filename(&oversized);
    assert!(sanitized.ends_with(".torrent"));
    assert!(sanitized.len() <= 255);
}

#[test]
fn persists_torrent_with_conflict_policy_and_atomic_temp_file() {
    let dir = tempdir().unwrap();
    let request = OutputRequest {
        output_dir: dir.path().to_path_buf(),
        filename: None,
        filename_hint: Some("unsafe/name".to_string()),
        conflict_policy: ConflictPolicy::Fail,
    };

    let path = persist_torrent(&request, b"d4:infod3:fooi1eee").unwrap();
    assert_eq!(path.file_name().unwrap(), "unsafe_name.torrent");
    assert_eq!(fs::read(&path).unwrap(), b"d4:infod3:fooi1eee");
    assert_eq!(temp_files(dir.path()).len(), 0);

    let conflict = persist_torrent(&request, b"d4:infod3:bari2eee").unwrap_err();
    assert_eq!(conflict.kind(), ErrorKind::OutputConflict);
    assert_eq!(fs::read(&path).unwrap(), b"d4:infod3:fooi1eee");

    let overwrite = OutputRequest {
        conflict_policy: ConflictPolicy::Overwrite,
        ..request
    };
    let overwritten = persist_torrent(&overwrite, b"d4:infod3:bari2eee").unwrap();
    assert_eq!(overwritten, path);
    assert_eq!(fs::read(&overwritten).unwrap(), b"d4:infod3:bari2eee");
}

#[test]
fn persist_torrent_requires_explicit_filename_source() {
    let dir = tempdir().unwrap();
    let request = OutputRequest {
        output_dir: dir.path().to_path_buf(),
        filename: None,
        filename_hint: None,
        conflict_policy: ConflictPolicy::Fail,
    };

    let error = persist_torrent(&request, b"d4:infod3:fooi1eee").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
}

#[test]
fn rejects_invalid_responses_before_final_rename() {
    let dir = tempdir().unwrap();
    let request = OutputRequest {
        output_dir: dir.path().to_path_buf(),
        filename: Some("bad.torrent".to_string()),
        filename_hint: None,
        conflict_policy: ConflictPolicy::Fail,
    };

    for bytes in [
        b"".as_slice(),
        b"<html>login</html>".as_slice(),
        b"not bencode".as_slice(),
    ] {
        let error = persist_torrent(&request, bytes).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::ParseFailure);
    }

    assert!(!dir.path().join("bad.torrent").exists());
    assert_eq!(temp_files(dir.path()).len(), 0);
}

#[test]
fn validates_torrent_response_size_and_bencode_shape() {
    validate_torrent_response(b"d4:infod3:fooi1eee").unwrap();
    assert_eq!(
        validate_torrent_response(b"").unwrap_err().kind(),
        ErrorKind::ParseFailure
    );
    assert_eq!(
        validate_torrent_response(b"<html>challenge</html>")
            .unwrap_err()
            .kind(),
        ErrorKind::ParseFailure
    );
    for invalid in [
        b"d4:infoXe".as_slice(),
        b"d4:infoiXee",
        b"d4:infoi-e",
        b"d4:infoi03ee",
    ] {
        assert_eq!(
            validate_torrent_response(invalid).unwrap_err().kind(),
            ErrorKind::ParseFailure
        );
    }

    let large = vec![b'd'; 20 * 1024 * 1024 + 1];
    assert_eq!(
        validate_torrent_response(&large).unwrap_err().kind(),
        ErrorKind::ParseFailure
    );
}

fn temp_files(path: &std::path::Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("tmp"))
        .collect()
}
