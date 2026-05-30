pub mod app;
pub mod auth;
pub mod categories;
pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod download;
pub mod error;
pub mod model;
pub mod output;
pub mod search;
pub mod show;

pub use app::{AppContext, DEFAULT_BASE_URL, RunConfig, TEST_BASE_URL, run};
pub use cli::{Cli, Commands};
pub use error::{ErrorKind, Result, TlError};
pub use model::{CategoryInfo, DownloadInfo, SearchResponse, SearchResult, TorrentDetails};
