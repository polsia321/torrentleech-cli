use std::path::PathBuf;

use clap::Parser;
use url::Url;

use crate::cli::{Cli, Commands};
use crate::config::{ConfigPaths, EnvConfig};
use crate::error::Result;

pub const DEFAULT_BASE_URL: &str = "https://www.torrentleech.org";
pub const TEST_BASE_URL: &str = "http://127.0.0.1";

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub base_url: Url,
    pub config_path: PathBuf,
}

impl RunConfig {
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let base_url = match cli.global.base_url.clone() {
            Some(url) => url,
            None => Url::parse(DEFAULT_BASE_URL).expect("default base URL is valid"),
        };
        let config_path = active_config_path(cli.global.config.clone());

        Ok(Self {
            base_url,
            config_path,
        })
    }
}

pub fn active_config_path(flag_path: Option<PathBuf>) -> PathBuf {
    flag_path
        .or_else(|| EnvConfig::current().config_path())
        .unwrap_or_else(|| ConfigPaths::defaults().config_path)
}

#[derive(Debug, Clone)]
pub struct AppContext {
    pub config: RunConfig,
}

impl AppContext {
    #[must_use]
    pub const fn new(config: RunConfig) -> Self {
        Self { config }
    }
}

pub fn run_from_env() -> Result<()> {
    run(Cli::parse())
}

pub fn run(cli: Cli) -> Result<()> {
    let config = RunConfig::from_cli(&cli)?;
    let context = AppContext::new(config);

    match cli.command {
        Commands::Login(args) => crate::commands::login::run(&context, args),
        Commands::Search(args) => crate::commands::search::run(&context, args),
        Commands::Categories(args) => crate::commands::categories::run(&context, args),
        Commands::Download(args) => crate::commands::download::run(&context, args),
        Commands::Show(args) => crate::commands::show::run(&context, args),
        Commands::Whoami(args) => crate::commands::whoami::run(&context, args),
        Commands::Logout(args) => crate::commands::logout::run(&context, args),
        Commands::Config(args) => crate::commands::config::run(&context, args),
    }
}
