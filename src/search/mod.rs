use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use url::Url;

use crate::categories::lookup_category;
use crate::cli::{SearchSort, SortOrder};
use crate::error::{ErrorKind, Result, TlError};
use crate::model::SearchResult;

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/');

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    pub query: Option<String>,
    pub categories: Vec<u32>,
    pub freeleech: bool,
    pub page: u32,
    pub sort: SearchSort,
    pub order: SortOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowsePage {
    pub total: Option<u32>,
    pub has_next_page: bool,
    pub results: Vec<SearchResult>,
}

pub fn build_search_url(base_url: &Url, request: &SearchRequest) -> Result<Url> {
    let mut path = String::from("/torrents/browse/index");

    if !request.categories.is_empty() {
        path.push_str("/categories/");
        path.push_str(
            &request
                .categories
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    if request.freeleech {
        path.push_str("/facets/tags%253AFREELEECH");
    }

    if let Some(query) = request.query.as_deref().filter(|query| !query.is_empty()) {
        path.push_str("/query/");
        path.push_str(&encode_path_segment(query));
    }

    path.push_str("/page/");
    path.push_str(&request.page.to_string());
    path.push_str("/orderby/");
    path.push_str(sort_segment(request.sort));
    path.push_str("/order/");
    path.push_str(order_segment(request.order));

    base_url
        .join(&path)
        .map_err(|error| TlError::with_source(ErrorKind::InvalidInput, "invalid base URL", error))
}

pub fn build_search_list_url(base_url: &Url, request: &SearchRequest) -> Result<Url> {
    let mut path = String::from("/torrents/browse/list");

    if !request.categories.is_empty() {
        path.push_str("/categories/");
        path.push_str(
            &request
                .categories
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    if request.freeleech {
        path.push_str("/facets/tags%3AFREELEECH");
    }

    if let Some(query) = request.query.as_deref().filter(|query| !query.is_empty()) {
        path.push_str("/query/");
        path.push_str(&encode_path_segment(query));
    }

    path.push_str("/orderby/");
    path.push_str(sort_segment(request.sort));
    path.push_str("/order/");
    path.push_str(order_segment(request.order));
    path.push_str("/page/");
    path.push_str(&request.page.to_string());

    base_url
        .join(&path)
        .map_err(|error| TlError::with_source(ErrorKind::InvalidInput, "invalid base URL", error))
}

pub fn parse_browse_json(input: &str, base_url: &Url) -> Result<BrowsePage> {
    let response: BrowseListResponse = serde_json::from_str(input).map_err(|error| {
        TlError::with_source(ErrorKind::ParseFailure, "browse JSON was invalid", error)
    })?;
    let per_page = response
        .per_page
        .unwrap_or(response.torrent_list.len() as u32);
    let has_next_page = response.page.saturating_mul(per_page) < response.num_found;
    let results = response
        .torrent_list
        .into_iter()
        .map(|torrent| torrent.into_result(base_url))
        .collect::<Result<Vec<_>>>()?;

    Ok(BrowsePage {
        total: Some(response.num_found),
        has_next_page,
        results,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowseListResponse {
    #[serde(rename = "numFound")]
    num_found: u32,
    #[serde(rename = "torrentList")]
    torrent_list: Vec<BrowseListTorrent>,
    page: u32,
    #[serde(rename = "perPage")]
    per_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowseListTorrent {
    fid: String,
    filename: String,
    name: String,
    #[serde(rename = "addedTimestamp")]
    added_timestamp: Option<String>,
    #[serde(rename = "categoryID")]
    category_id: Option<u32>,
    size: u64,
    completed: Option<u32>,
    seeders: Option<u32>,
    leechers: Option<u32>,
    #[serde(rename = "numComments")]
    num_comments: Option<u32>,
    #[serde(default)]
    tags: BrowseTags,
    #[serde(default)]
    download_multiplier: Option<u32>,
    uploader: Option<String>,
}

impl BrowseListTorrent {
    fn into_result(self, base_url: &Url) -> Result<SearchResult> {
        let id = self.fid.parse::<u64>().map_err(|error| {
            TlError::with_source(ErrorKind::ParseFailure, "torrent id was invalid", error)
        })?;
        let download_url = base_url
            .join(&format!(
                "/download/{id}/{}",
                encode_path_segment(&self.filename)
            ))
            .map_err(|error| {
                TlError::with_source(ErrorKind::ParseFailure, "invalid download URL", error)
            })?;
        let category = self
            .category_id
            .and_then(|id| lookup_category(id).ok())
            .map(|category| format!("{}/{}", category.group, category.name));
        let tags = self.tags.into_vec();
        let freeleech = self.download_multiplier == Some(0)
            || tags.iter().any(|tag| tag.eq_ignore_ascii_case("FREELEECH"));

        Ok(SearchResult {
            id,
            title: self.name,
            category_id: self.category_id,
            category,
            tags,
            added: self.added_timestamp,
            comments: self.num_comments,
            size: Some(human_file_size(self.size)),
            completed: self.completed,
            seeders: self.seeders,
            leechers: self.leechers,
            uploader: self.uploader,
            download_url,
            freeleech,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(untagged)]
enum BrowseTags {
    List(Vec<String>),
    Text(String),
    #[default]
    Empty,
}

impl BrowseTags {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::List(tags) => tags,
            Self::Text(text) if text.trim().is_empty() => Vec::new(),
            Self::Text(text) => vec![text],
            Self::Empty => Vec::new(),
        }
    }
}

fn human_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

pub fn parse_browse_html(html: &str, base_url: &Url) -> Result<BrowsePage> {
    let document = Html::parse_document(html);
    let table_selector = selector("#torrenttable")?;
    if document.select(&table_selector).next().is_none() {
        return Err(parse_error("browse page did not contain a torrent table"));
    }

    let total = parse_total(&document)?;
    let row_selector = selector("#torrenttable tbody tr")?;
    let rows: Vec<_> = document.select(&row_selector).collect();
    if rows.is_empty() && total != Some(0) {
        return Err(parse_error("browse table did not contain result rows"));
    }

    Ok(BrowsePage {
        total,
        has_next_page: has_next_page(&document)?,
        results: rows
            .into_iter()
            .map(|row| parse_row(row, base_url))
            .collect::<Result<Vec<_>>>()?,
    })
}

fn parse_row(row: ElementRef<'_>, base_url: &Url) -> Result<SearchResult> {
    let cell_selector = selector("td")?;
    let cells: Vec<_> = row.select(&cell_selector).collect();
    if cells.len() < 7 {
        return Err(parse_error("browse row did not contain expected columns"));
    }

    let link_selector = selector("a[href]")?;
    let tag_selector = selector(".tag")?;
    let uploader_selector = selector(".uploader")?;

    let title_link = cells[0]
        .select(&link_selector)
        .find(|link| link.value().attr("href").is_some_and(is_torrent_href))
        .ok_or_else(|| parse_error("browse row did not contain a torrent link"))?;
    let title_href = title_link
        .value()
        .attr("href")
        .ok_or_else(|| parse_error("torrent link did not contain href"))?;
    let id = parse_torrent_id(title_href)?;
    let title = text(&title_link);
    if title.is_empty() {
        return Err(parse_error("browse row did not contain a title"));
    }

    let download_link = cells[0]
        .select(&link_selector)
        .find(|link| link.value().attr("href").is_some_and(is_download_href))
        .ok_or_else(|| parse_error("browse row did not contain a download link"))?;
    let download_href = download_link
        .value()
        .attr("href")
        .ok_or_else(|| parse_error("download link did not contain href"))?;
    let download_url = base_url.join(download_href).map_err(|error| {
        TlError::with_source(ErrorKind::ParseFailure, "invalid download URL", error)
    })?;

    let category_link = cells[0].select(&link_selector).find(|link| {
        link.value()
            .attr("href")
            .is_some_and(|href| href.contains("/categories/"))
    });
    let category = category_link
        .as_ref()
        .map(text)
        .filter(|value| !value.is_empty());
    let category_id = category_link
        .and_then(|link| link.value().attr("href"))
        .and_then(parse_category_id);

    let tags: Vec<String> = cells[0]
        .select(&tag_selector)
        .map(|tag| text(&tag))
        .filter(|tag| !tag.is_empty())
        .collect();
    let freeleech = tags.iter().any(|tag| tag.eq_ignore_ascii_case("FREELEECH"));
    let uploader = cells[0]
        .select(&uploader_selector)
        .next()
        .map(|element| text(&element))
        .filter(|value| !value.is_empty());

    Ok(SearchResult {
        id,
        title,
        category_id,
        category,
        tags,
        added: non_empty_text(cells[1]),
        comments: parse_optional_u32(&text(&cells[2]))?,
        size: non_empty_text(cells[3]),
        completed: parse_optional_u32(&text(&cells[4]))?,
        seeders: parse_optional_u32(&text(&cells[5]))?,
        leechers: parse_optional_u32(&text(&cells[6]))?,
        uploader,
        download_url,
        freeleech,
    })
}

fn has_next_page(document: &Html) -> Result<bool> {
    let selector = selector("a[href]")?;
    Ok(document.select(&selector).any(|link| {
        link.value().attr("rel") == Some("next")
            || (text(&link).eq_ignore_ascii_case("next")
                && link
                    .value()
                    .attr("href")
                    .is_some_and(|href| href.contains("/page/")))
    }))
}

fn parse_total(document: &Html) -> Result<Option<u32>> {
    let selector = selector(".browse-header")?;
    let Some(header) = document
        .select(&selector)
        .next()
        .map(|element| text(&element))
    else {
        return Ok(None);
    };

    let normalized = header.to_ascii_lowercase();
    if normalized.contains("0 results") {
        return Ok(Some(0));
    }

    for marker in [" of ", "total ", "found "] {
        let Some(start) = normalized.find(marker).map(|index| index + marker.len()) else {
            continue;
        };
        let digits: String = normalized[start..]
            .chars()
            .skip_while(|character| !character.is_ascii_digit())
            .take_while(|character| character.is_ascii_digit() || *character == ',')
            .filter(|character| character.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            continue;
        }
        return digits.parse::<u32>().map(Some).map_err(|error| {
            TlError::with_source(ErrorKind::ParseFailure, "total count was invalid", error)
        });
    }

    Ok(None)
}

fn parse_torrent_id(href: &str) -> Result<u64> {
    href.split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .as_slice()
        .windows(2)
        .find_map(|window| (window[0] == "torrent").then_some(window[1]))
        .ok_or_else(|| parse_error("torrent link did not contain an id"))?
        .parse::<u64>()
        .map_err(|error| {
            TlError::with_source(ErrorKind::ParseFailure, "torrent id was invalid", error)
        })
}

fn parse_category_id(href: &str) -> Option<u32> {
    let segments: Vec<_> = href
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    segments
        .windows(2)
        .find_map(|window| (window[0] == "categories").then_some(window[1]))
        .and_then(|id| id.parse::<u32>().ok())
}

fn parse_optional_u32(input: &str) -> Result<Option<u32>> {
    let normalized = input.trim().replace(',', "");
    if normalized.is_empty() {
        return Ok(None);
    }
    normalized.parse::<u32>().map(Some).map_err(|error| {
        TlError::with_source(
            ErrorKind::ParseFailure,
            "numeric browse column was invalid",
            error,
        )
    })
}

fn is_torrent_href(href: &str) -> bool {
    href.split('/').any(|segment| segment == "torrent")
}

fn is_download_href(href: &str) -> bool {
    href.split('/').any(|segment| segment == "download")
}

fn non_empty_text(element: ElementRef<'_>) -> Option<String> {
    let value = text(&element);
    (!value.is_empty()).then_some(value)
}

fn text(element: &ElementRef<'_>) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn encode_path_segment(input: &str) -> String {
    utf8_percent_encode(input, PATH_SEGMENT_ENCODE_SET).to_string()
}

fn sort_segment(sort: SearchSort) -> &'static str {
    match sort {
        SearchSort::Added => "added",
        SearchSort::Size => "size",
        SearchSort::Seeders => "seeders",
        SearchSort::Leechers => "leechers",
        SearchSort::Completed => "completed",
        SearchSort::Comments => "numComments",
        SearchSort::Name => "nameSort",
    }
}

fn order_segment(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Asc => "asc",
        SortOrder::Desc => "desc",
    }
}

fn selector(input: &str) -> Result<Selector> {
    Selector::parse(input).map_err(|error| {
        TlError::new(
            ErrorKind::ParseFailure,
            format!("browse selector is invalid: {error}"),
        )
    })
}

fn parse_error(message: impl Into<String>) -> TlError {
    TlError::new(ErrorKind::ParseFailure, message)
}
