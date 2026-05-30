use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use percent_encoding::percent_decode_str;
use scraper::{Html, Selector};
use url::Url;

use crate::error::{ErrorKind, Result, TlError};

const MAX_FILENAME_LEN: usize = 255;
const MAX_TORRENT_BYTES: usize = 20 * 1024 * 1024;
const TORRENT_EXTENSION: &str = ".torrent";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    pub id: u64,
    pub url: Url,
    pub filename_hint: Option<String>,
    pub kind: TargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Detail,
    DirectDownload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputRequest {
    pub output_dir: PathBuf,
    pub filename: Option<String>,
    pub filename_hint: Option<String>,
    pub conflict_policy: ConflictPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictPolicy {
    Fail,
    Overwrite,
}

#[must_use]
pub fn sanitize_torrent_filename(input: &str) -> String {
    let replaced: String = input
        .chars()
        .map(|character| match character {
            '/' | '\\' | '\0' | '\t' | '\n' | '\r' => '_',
            _ => character,
        })
        .collect();
    let stripped = replaced.trim_start_matches('.');
    let named = if stripped.is_empty() {
        "download".to_string()
    } else {
        stripped.to_string()
    };
    let with_extension = if named.ends_with(TORRENT_EXTENSION) {
        named
    } else {
        format!("{named}{TORRENT_EXTENSION}")
    };

    truncate_filename(with_extension)
}

pub fn resolve_target(target: &str, base_url: &Url) -> Result<ResolvedTarget> {
    if let Ok(id) = target.parse::<u64>() {
        let url = base_url.join(&format!("/torrent/{id}")).map_err(|error| {
            TlError::with_source(ErrorKind::InvalidInput, "invalid base URL", error)
        })?;
        return Ok(ResolvedTarget {
            id,
            url,
            filename_hint: None,
            kind: TargetKind::Detail,
        });
    }

    let url = Url::parse(target).map_err(|error| {
        TlError::with_source(
            ErrorKind::InvalidInput,
            "target must be a torrent id or URL",
            error,
        )
    })?;
    ensure_allowed_host(&url, base_url)?;

    let segments = path_segments(&url);
    match segments.as_slice() {
        ["torrent", id] => {
            let id = parse_id(id)?;
            Ok(ResolvedTarget {
                id,
                url,
                filename_hint: None,
                kind: TargetKind::Detail,
            })
        }
        ["download", id, filename] => {
            let id = parse_id(id)?;
            let filename_hint = percent_decode(filename);
            Ok(ResolvedTarget {
                id,
                url,
                filename_hint: Some(filename_hint),
                kind: TargetKind::DirectDownload,
            })
        }
        _ => Err(TlError::new(
            ErrorKind::InvalidInput,
            "URL must be a TorrentLeech torrent or download URL",
        )),
    }
}

pub fn parse_detail_download_link(html: &str, id: u64, base_url: &Url) -> Result<Url> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").map_err(|error| {
        TlError::new(
            ErrorKind::ParseFailure,
            format!("download link selector is invalid: {error}"),
        )
    })?;

    for element in document.select(&selector) {
        let Some(href) = element.value().attr("href") else {
            continue;
        };
        let url = base_url.join(href).map_err(|error| {
            TlError::with_source(ErrorKind::ParseFailure, "invalid download link", error)
        })?;
        if ensure_allowed_host(&url, base_url).is_err() {
            continue;
        }
        let segments = path_segments(&url);
        if matches!(segments.as_slice(), ["download", link_id, _] if parse_id(link_id).ok() == Some(id))
        {
            return Ok(url);
        }
    }

    Err(TlError::new(
        ErrorKind::ParseFailure,
        "detail page did not contain a matching download link",
    ))
}

pub fn validate_torrent_response(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        return Err(TlError::new(
            ErrorKind::ParseFailure,
            "torrent response was empty",
        ));
    }
    if bytes.len() > MAX_TORRENT_BYTES {
        return Err(TlError::new(
            ErrorKind::ParseFailure,
            "torrent response was too large",
        ));
    }

    let trimmed = trim_ascii_whitespace(bytes);
    if looks_like_html(trimmed) || !looks_like_torrent_bencode(trimmed) {
        return Err(TlError::new(
            ErrorKind::ParseFailure,
            "torrent response was not a torrent file",
        ));
    }

    Ok(())
}

pub fn persist_torrent(request: &OutputRequest, bytes: &[u8]) -> Result<PathBuf> {
    validate_torrent_response(bytes)?;

    fs::create_dir_all(&request.output_dir).map_err(|error| {
        TlError::with_source(
            ErrorKind::Unexpected,
            "failed to create output directory",
            error,
        )
    })?;

    let selected = request
        .filename
        .as_deref()
        .or(request.filename_hint.as_deref())
        .ok_or_else(|| {
            TlError::new(
                ErrorKind::InvalidInput,
                "download filename was not available",
            )
        })?;
    let filename = sanitize_torrent_filename(selected);
    let destination = request.output_dir.join(filename);

    if destination.exists() && request.conflict_policy == ConflictPolicy::Fail {
        return Err(TlError::new(
            ErrorKind::OutputConflict,
            format!("output file already exists: {}", destination.display()),
        ));
    }

    let temp_path = write_temp_file(request, bytes)?;
    match request.conflict_policy {
        ConflictPolicy::Fail => persist_without_overwrite(&temp_path, &destination)?,
        ConflictPolicy::Overwrite => rename_overwrite(&temp_path, &destination)?,
    }

    Ok(destination)
}

fn persist_without_overwrite(
    temp_path: &std::path::Path,
    destination: &std::path::Path,
) -> Result<()> {
    match fs::hard_link(temp_path, destination) {
        Ok(()) => {
            fs::remove_file(temp_path).map_err(|error| {
                TlError::with_source(
                    ErrorKind::Unexpected,
                    "failed to remove temporary file",
                    error,
                )
            })?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(temp_path);
            Err(TlError::new(
                ErrorKind::OutputConflict,
                format!("output file already exists: {}", destination.display()),
            ))
        }
        Err(error) => {
            let _ = fs::remove_file(temp_path);
            Err(TlError::with_source(
                ErrorKind::Unexpected,
                "failed to move torrent into place",
                error,
            ))
        }
    }
}

fn rename_overwrite(temp_path: &std::path::Path, destination: &std::path::Path) -> Result<()> {
    if let Err(error) = fs::rename(temp_path, destination) {
        let _ = fs::remove_file(temp_path);
        return Err(TlError::with_source(
            ErrorKind::Unexpected,
            "failed to move torrent into place",
            error,
        ));
    }
    Ok(())
}

fn write_temp_file(request: &OutputRequest, bytes: &[u8]) -> Result<PathBuf> {
    for counter in 0..1000 {
        let temp_path = request
            .output_dir
            .join(format!(".tl-download-{}-{counter}.tmp", std::process::id()));
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(TlError::with_source(
                    ErrorKind::Unexpected,
                    "failed to create temporary torrent file",
                    error,
                ));
            }
        };

        if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
            let _ = fs::remove_file(&temp_path);
            return Err(TlError::with_source(
                ErrorKind::Unexpected,
                "failed to write temporary torrent file",
                error,
            ));
        }

        return Ok(temp_path);
    }

    Err(TlError::new(
        ErrorKind::Unexpected,
        "failed to allocate temporary torrent filename",
    ))
}

fn ensure_allowed_host(url: &Url, base_url: &Url) -> Result<()> {
    if url.scheme() != base_url.scheme() || url.host_str() != base_url.host_str() {
        return Err(TlError::new(
            ErrorKind::InvalidInput,
            "URL host must match the configured TorrentLeech host",
        ));
    }
    Ok(())
}

fn parse_id(input: &str) -> Result<u64> {
    input.parse::<u64>().map_err(|error| {
        TlError::with_source(
            ErrorKind::InvalidInput,
            "torrent id must be a number",
            error,
        )
    })
}

fn path_segments(url: &Url) -> Vec<&str> {
    url.path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default()
}

fn percent_decode(input: &str) -> String {
    percent_decode_str(input).decode_utf8_lossy().into_owned()
}

fn truncate_filename(filename: String) -> String {
    if filename.len() <= MAX_FILENAME_LEN {
        return filename;
    }

    let stem_limit = MAX_FILENAME_LEN - TORRENT_EXTENSION.len();
    let stem = filename.trim_end_matches(TORRENT_EXTENSION);
    let mut truncated = String::new();
    for character in stem.chars() {
        if truncated.len() + character.len_utf8() > stem_limit {
            break;
        }
        truncated.push(character);
    }
    truncated.push_str(TORRENT_EXTENSION);
    truncated
}

fn trim_ascii_whitespace(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|position| position + 1)
        .unwrap_or(start);
    &bytes[start..end]
}

fn looks_like_html(bytes: &[u8]) -> bool {
    let prefix_len = bytes.len().min(32);
    let prefix = String::from_utf8_lossy(&bytes[..prefix_len]).to_ascii_lowercase();
    prefix.starts_with("<!doctype html") || prefix.starts_with("<html")
}

fn looks_like_torrent_bencode(bytes: &[u8]) -> bool {
    let Some(entries) = parse_dictionary(bytes) else {
        return false;
    };

    entries.iter().any(|(key, _)| *key == b"info")
}

fn parse_dictionary(bytes: &[u8]) -> Option<Vec<(&[u8], &[u8])>> {
    let mut cursor = 0;
    if bytes.get(cursor) != Some(&b'd') {
        return None;
    }
    cursor += 1;

    let mut entries = Vec::new();
    while bytes.get(cursor) != Some(&b'e') {
        let key = parse_bytes(bytes, &mut cursor)?;
        let value_start = cursor;
        skip_value(bytes, &mut cursor)?;
        entries.push((key, &bytes[value_start..cursor]));
    }
    cursor += 1;

    (cursor == bytes.len()).then_some(entries)
}

fn skip_value(bytes: &[u8], cursor: &mut usize) -> Option<()> {
    match bytes.get(*cursor)? {
        b'i' => skip_integer(bytes, cursor),
        b'l' => skip_list(bytes, cursor),
        b'd' => skip_dictionary(bytes, cursor),
        b'0'..=b'9' => parse_bytes(bytes, cursor).map(|_| ()),
        _ => None,
    }
}

fn skip_integer(bytes: &[u8], cursor: &mut usize) -> Option<()> {
    *cursor += 1;
    let start = *cursor;
    if bytes.get(*cursor) == Some(&b'-') {
        *cursor += 1;
    }
    let digit_start = *cursor;
    while matches!(bytes.get(*cursor), Some(b'0'..=b'9')) {
        *cursor += 1;
    }
    if digit_start == *cursor || bytes.get(*cursor) != Some(&b'e') {
        return None;
    }
    let digits = &bytes[digit_start..*cursor];
    if bytes.get(start) == Some(&b'-') && digits == b"0" {
        return None;
    }
    if digits.len() > 1 && digits.first() == Some(&b'0') {
        return None;
    }
    *cursor += 1;
    Some(())
}

fn skip_list(bytes: &[u8], cursor: &mut usize) -> Option<()> {
    *cursor += 1;
    while bytes.get(*cursor) != Some(&b'e') {
        skip_value(bytes, cursor)?;
    }
    *cursor += 1;
    Some(())
}

fn skip_dictionary(bytes: &[u8], cursor: &mut usize) -> Option<()> {
    *cursor += 1;
    while bytes.get(*cursor) != Some(&b'e') {
        parse_bytes(bytes, cursor)?;
        skip_value(bytes, cursor)?;
    }
    *cursor += 1;
    Some(())
}

fn parse_bytes<'a>(bytes: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    let start = *cursor;
    while matches!(bytes.get(*cursor), Some(b'0'..=b'9')) {
        *cursor += 1;
    }
    if start == *cursor || bytes.get(*cursor) != Some(&b':') {
        return None;
    }
    let length = std::str::from_utf8(&bytes[start..*cursor])
        .ok()?
        .parse::<usize>()
        .ok()?;
    *cursor += 1;
    let end = cursor.checked_add(length)?;
    let value = bytes.get(*cursor..end)?;
    *cursor = end;
    Some(value)
}
