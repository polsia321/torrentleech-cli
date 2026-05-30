use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::download::{parse_detail_download_link, resolve_target};
use crate::error::{ErrorKind, Result, TlError};
use crate::model::TorrentDetails;

pub fn parse_torrent_details(html: &str, target: &str, base_url: &Url) -> Result<TorrentDetails> {
    let resolved = resolve_target(target, base_url)?;
    let document = Html::parse_document(html);
    let title = parse_title(&document)?;
    let table = parse_info_table(&document)?;
    let download_url = parse_detail_download_link(html, resolved.id, base_url)?;

    Ok(TorrentDetails {
        id: resolved.id,
        title,
        category: table.category,
        added: table.added,
        size: table.size,
        seeders: table.seeders,
        leechers: table.leechers,
        completed: table.completed,
        uploader: table.uploader,
        tags: table.tags,
        description: table.description,
        nfo: parse_nfo(&document)?,
        download_url,
    })
}

#[derive(Debug, Default)]
struct InfoTable {
    category: Option<String>,
    added: Option<String>,
    size: Option<String>,
    seeders: Option<u32>,
    leechers: Option<u32>,
    completed: Option<u32>,
    uploader: Option<String>,
    tags: Vec<String>,
    description: Option<String>,
}

fn parse_title(document: &Html) -> Result<String> {
    for selector in ["#torrentnameid", "h2.page-heading", "title"] {
        let element_selector = selector_for(selector)?;
        let Some(element) = document.select(&element_selector).next() else {
            continue;
        };
        let mut title = text(&element);
        if selector == "title" {
            title = title
                .strip_prefix("Torrent Details for ")
                .unwrap_or(&title)
                .split(" :: ")
                .next()
                .unwrap_or(&title)
                .to_string();
        }
        title = title.trim_end_matches(" FREELEECH").trim().to_string();
        if !title.is_empty() {
            return Ok(title);
        }
    }

    Err(TlError::new(
        ErrorKind::ParseFailure,
        "detail page did not contain a title",
    ))
}

fn parse_info_table(document: &Html) -> Result<InfoTable> {
    let row_selector = selector_for(".torrent_info_details tr, #torrentinfo tr")?;
    let label_selector = selector_for("td.description")?;
    let cell_selector = selector_for("td")?;
    let tag_selector = selector_for(".tag")?;
    let uploader_selector = selector_for(".details-uploader-name")?;
    let mut table = InfoTable::default();

    for row in document.select(&row_selector) {
        let Some(label_cell) = row.select(&label_selector).next() else {
            continue;
        };
        let label = normalize_label(&text(&label_cell));
        let cells: Vec<_> = row.select(&cell_selector).collect();
        if cells.len() < 2 {
            continue;
        }
        let value_cell = cells[1];
        let value = text(&value_cell);

        match label.as_str() {
            "category" => table.category = (!value.is_empty()).then_some(value),
            "added" => table.added = (!value.is_empty()).then_some(clean_added(&value)),
            "size" => table.size = (!value.is_empty()).then_some(value),
            "peers" => {
                if table.seeders.is_none() {
                    table.seeders = parse_count_before(&value, "seeder");
                }
                if table.leechers.is_none() {
                    table.leechers = parse_count_before(&value, "leecher");
                }
            }
            "downloaded" => table.completed = parse_first_u32(&value),
            "uploader" => {
                table.uploader = value_cell
                    .select(&uploader_selector)
                    .next()
                    .map(|element| text(&element))
                    .filter(|value| !value.is_empty())
                    .or_else(|| (!value.is_empty()).then_some(value));
            }
            "comments" => table.description = non_empty_not_none(&value),
            "uploadercomments" => {
                if table.description.is_none() {
                    table.description = non_empty_not_none(&value);
                }
            }
            "tags" => {
                table.tags = value_cell
                    .select(&tag_selector)
                    .map(|tag| text(&tag))
                    .filter(|tag| !tag.is_empty())
                    .collect();
            }
            "seeders" => table.seeders = parse_first_u32(&value),
            "leechers" => table.leechers = parse_first_u32(&value),
            _ => {}
        }
    }

    Ok(table)
}

fn parse_nfo(document: &Html) -> Result<Option<String>> {
    for selector in ["#nfo_text", "pre.nfo"] {
        let element_selector = selector_for(selector)?;
        let Some(element) = document.select(&element_selector).next() else {
            continue;
        };
        let value = text_preserve_lines(&element);
        if !value.is_empty() {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn clean_added(input: &str) -> String {
    input
        .split_once('(')
        .map(|(before, _)| before)
        .unwrap_or(input)
        .trim()
        .to_string()
}

fn non_empty_not_none(input: &str) -> Option<String> {
    let trimmed = input.trim();
    (!trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none")).then(|| trimmed.to_string())
}

fn parse_count_before(input: &str, marker: &str) -> Option<u32> {
    let normalized = input.to_ascii_lowercase();
    let marker_start = normalized.find(marker)?;
    let before = &normalized[..marker_start];
    before
        .split(|character: char| !character.is_ascii_digit() && character != ',')
        .filter(|part| !part.is_empty())
        .next_back()
        .and_then(parse_u32)
}

fn parse_first_u32(input: &str) -> Option<u32> {
    input
        .split(|character: char| !character.is_ascii_digit() && character != ',')
        .find(|part| !part.is_empty())
        .and_then(parse_u32)
}

fn parse_u32(input: &str) -> Option<u32> {
    input.replace(',', "").parse().ok()
}

fn normalize_label(input: &str) -> String {
    input
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn text_preserve_lines(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join("")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn selector_for(input: &str) -> Result<Selector> {
    Selector::parse(input).map_err(|error| {
        TlError::new(
            ErrorKind::ParseFailure,
            format!("detail selector is invalid: {error}"),
        )
    })
}
