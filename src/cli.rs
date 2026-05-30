use std::path::PathBuf;

use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args, Parser, Subcommand, ValueEnum};
use url::Url;

const STYLES: Styles = Styles::styled()
    .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default());

#[derive(Debug, Clone, Parser)]
#[command(name = "tl")]
#[command(about = "Command line client for TorrentLeech")]
#[command(styles = STYLES)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Clone, Args)]
pub struct GlobalArgs {
    #[arg(long, global = true, hide = true)]
    pub base_url: Option<Url>,

    #[arg(long, global = true, env = "TL_CONFIG")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    Login(LoginArgs),
    Search(SearchArgs),
    Categories(CategoriesArgs),
    Download(DownloadArgs),
    Show(ShowArgs),
    Whoami(WhoamiArgs),
    Logout(LogoutArgs),
    Config(ConfigArgs),
}

#[derive(Debug, Clone, Args)]
pub struct LoginArgs {
    #[arg(short, long, env = "TL_USERNAME")]
    pub username: Option<String>,

    #[arg(long)]
    pub password_stdin: bool,

    #[arg(long, env = "TL_PASSWORD_FILE")]
    pub password_file: Option<PathBuf>,

    #[arg(long, env = "TL_COOKIE_JAR")]
    pub cookie_jar: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct SearchArgs {
    pub query: Option<String>,

    #[arg(short, long = "category", value_name = "CATEGORY")]
    pub categories: Vec<String>,

    #[arg(long)]
    pub freeleech: bool,

    #[arg(short, long)]
    pub limit: Option<u32>,

    #[arg(long, default_value_t = 1)]
    pub page: u32,

    #[arg(long, value_enum, default_value_t = SearchSort::Added)]
    pub sort: SearchSort,

    #[arg(long, value_enum, default_value_t = SortOrder::Desc)]
    pub order: SortOrder,

    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SearchSort {
    Added,
    Size,
    Seeders,
    Leechers,
    Completed,
    Comments,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Args)]
pub struct CategoriesArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct DownloadArgs {
    pub target: String,

    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,

    #[arg(long)]
    pub filename: Option<String>,

    #[arg(long)]
    pub force: bool,

    #[arg(long)]
    pub print_path: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    pub target: String,

    #[arg(long)]
    pub description_only: bool,

    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct WhoamiArgs {
    #[arg(long, env = "TL_COOKIE_JAR")]
    pub cookie_jar: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct LogoutArgs {
    #[arg(long, env = "TL_COOKIE_JAR")]
    pub cookie_jar: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    Init(ConfigInitArgs),
    Path(ConfigPathArgs),
    Show(ConfigShowArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ConfigInitArgs {
    #[arg(long)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigPathArgs {}

#[derive(Debug, Clone, Args)]
pub struct ConfigShowArgs {
    #[arg(long)]
    pub json: bool,
}
