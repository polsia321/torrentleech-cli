use time::{
    Duration, OffsetDateTime, PrimitiveDateTime, format_description::well_known::Iso8601,
    macros::format_description,
};

use crate::error::{ErrorKind, Result, TlError};
use serde::Serialize;

use crate::model::{SearchResponse, SearchResult, TorrentDetails};

pub fn render_search_compact(response: &SearchResponse, now: OffsetDateTime) -> Result<String> {
    if response.results.is_empty() {
        return Ok(String::new());
    }

    let mut lines = Vec::with_capacity(response.results.len());
    for result in &response.results {
        let category = result.category.as_deref().unwrap_or("-");
        let size = result.size.as_deref().unwrap_or("-");
        let seeders = result
            .seeders
            .map_or_else(|| "-".to_string(), |value| value.to_string());
        let leechers = result
            .leechers
            .map_or_else(|| "-".to_string(), |value| value.to_string());
        let age = match result.added.as_deref() {
            Some(added) => format_age(parse_added(added)?, now),
            None => "-".to_string(),
        };
        let freeleech = if result.freeleech { "FL" } else { "-" };

        lines.push(format!(
            "{} {age} s{seeders} l{leechers} {size} {category} {freeleech} {}",
            result.id, result.title
        ));
    }

    Ok(format!("{}\n", lines.join("\n")))
}

pub fn render_search_json(response: &SearchResponse) -> Result<String> {
    serde_json::to_string_pretty(&StableSearchResponse::from(response)).map_err(|error| {
        TlError::with_source(ErrorKind::Unexpected, "failed to render search JSON", error)
    })
}

pub fn render_show_compact(details: &TorrentDetails) -> String {
    let mut lines = vec![format!("{} {}", details.id, details.title)];
    if let Some(category) = &details.category {
        lines.push(format!("category: {category}"));
    }
    if let Some(size) = &details.size {
        lines.push(format!("size: {size}"));
    }
    if let Some(seeders) = details.seeders {
        lines.push(format!("seeders: {seeders}"));
    }
    if let Some(leechers) = details.leechers {
        lines.push(format!("leechers: {leechers}"));
    }
    if let Some(description) = details
        .description
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        lines.push(String::new());
        lines.push(description.to_string());
    }
    if let Some(nfo) = details.nfo.as_deref().filter(|value| !value.is_empty()) {
        lines.push(String::new());
        lines.push(nfo.to_string());
    }
    format!("{}\n", lines.join("\n"))
}

pub fn render_show_description(details: &TorrentDetails) -> String {
    let text = details
        .description
        .as_deref()
        .or(details.nfo.as_deref())
        .unwrap_or_default();
    if text.is_empty() {
        String::new()
    } else {
        format!("{text}\n")
    }
}

pub fn render_show_json(details: &TorrentDetails) -> Result<String> {
    serde_json::to_string_pretty(details).map_err(|error| {
        TlError::with_source(ErrorKind::Unexpected, "failed to render show JSON", error)
    })
}

#[derive(Serialize)]
struct StableSearchResponse<'a> {
    query: &'a Option<String>,
    page: u32,
    total: Option<u32>,
    results: Vec<StableSearchResult<'a>>,
}

impl<'a> From<&'a SearchResponse> for StableSearchResponse<'a> {
    fn from(response: &'a SearchResponse) -> Self {
        Self {
            query: &response.query,
            page: response.page,
            total: response.total,
            results: response
                .results
                .iter()
                .map(StableSearchResult::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct StableSearchResult<'a> {
    id: u64,
    title: &'a str,
    category_id: Option<u32>,
    category: &'a Option<String>,
    tags: &'a [String],
    added: &'a Option<String>,
    comments: Option<u32>,
    size: &'a Option<String>,
    completed: Option<u32>,
    seeders: Option<u32>,
    leechers: Option<u32>,
    download_url: &'a url::Url,
}

impl<'a> From<&'a SearchResult> for StableSearchResult<'a> {
    fn from(result: &'a SearchResult) -> Self {
        Self {
            id: result.id,
            title: &result.title,
            category_id: result.category_id,
            category: &result.category,
            tags: &result.tags,
            added: &result.added,
            comments: result.comments,
            size: &result.size,
            completed: result.completed,
            seeders: result.seeders,
            leechers: result.leechers,
            download_url: &result.download_url,
        }
    }
}

fn parse_added(input: &str) -> Result<OffsetDateTime> {
    if let Ok(value) = OffsetDateTime::parse(input, &Iso8601::DEFAULT) {
        return Ok(value);
    }

    let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    PrimitiveDateTime::parse(input, &format)
        .map(|value| value.assume_utc())
        .map_err(|error| {
            TlError::with_source(ErrorKind::ParseFailure, "invalid added timestamp", error)
        })
}

fn format_age(added: OffsetDateTime, now: OffsetDateTime) -> String {
    let elapsed = now - added;
    let elapsed = if elapsed < Duration::ZERO {
        Duration::ZERO
    } else {
        elapsed
    };
    let minutes = elapsed.whole_minutes();

    if minutes < 60 {
        format!("{minutes}m")
    } else if minutes < 48 * 60 {
        format!("{}h", minutes / 60)
    } else if minutes < 60 * 24 * 60 {
        format!("{}d", minutes / (24 * 60))
    } else if minutes < 2 * 365 * 24 * 60 {
        format!("{}mo", minutes / (30 * 24 * 60))
    } else {
        format!("{}y", minutes / (365 * 24 * 60))
    }
}
