use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: Option<String>,
    pub page: u32,
    pub total: Option<u32>,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: u64,
    pub title: String,
    pub category_id: Option<u32>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub added: Option<String>,
    pub comments: Option<u32>,
    pub size: Option<String>,
    pub completed: Option<u32>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    pub uploader: Option<String>,
    pub download_url: Url,
    pub freeleech: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub id: u64,
    pub filename: String,
    pub source_url: Url,
    pub saved_path: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TorrentDetails {
    pub id: u64,
    pub title: String,
    pub category: Option<String>,
    pub added: Option<String>,
    pub size: Option<String>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    pub completed: Option<u32>,
    pub uploader: Option<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub nfo: Option<String>,
    pub download_url: Url,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryInfo {
    pub id: u32,
    pub name: String,
    pub group: String,
    pub aliases: Vec<String>,
}
