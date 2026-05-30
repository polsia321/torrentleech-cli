use clap::{CommandFactory, Parser};
use torrentleech_cli::app::{DEFAULT_BASE_URL, RunConfig, TEST_BASE_URL};
use torrentleech_cli::cli::{Cli, Commands, ConfigCommand};
use torrentleech_cli::error::ErrorKind;
use torrentleech_cli::{CategoryInfo, DownloadInfo, SearchResponse, SearchResult, TorrentDetails};
use url::Url;

#[test]
fn top_level_help_lists_foundation_commands() {
    let mut output = Vec::new();
    Cli::command().write_help(&mut output).unwrap();
    let help = String::from_utf8(output).unwrap();

    for command in [
        "login",
        "search",
        "categories",
        "download",
        "show",
        "whoami",
        "logout",
        "config",
    ] {
        assert!(help.contains(command), "missing {command} in help:\n{help}");
    }
}

#[test]
fn search_query_is_optional_for_category_browsing() {
    let cli = Cli::parse_from(["tl", "search", "--category", "movies"]);
    let Commands::Search(args) = cli.command else {
        panic!("expected search command");
    };

    assert_eq!(args.query, None);
    assert_eq!(args.categories, vec!["movies"]);
}

#[test]
fn json_flags_are_command_local() {
    assert!(Cli::try_parse_from(["tl", "categories", "--json"]).is_ok());
    assert!(Cli::try_parse_from(["tl", "search", "--json"]).is_ok());
    assert!(Cli::try_parse_from(["tl", "config", "show", "--json"]).is_ok());
    assert!(Cli::try_parse_from(["tl", "show", "123", "--json"]).is_ok());
    assert!(Cli::try_parse_from(["tl", "download", "123", "--json"]).is_err());
}

#[test]
fn config_subcommands_are_stable() {
    let cli = Cli::parse_from(["tl", "config", "path"]);
    let Commands::Config(args) = cli.command else {
        panic!("expected config command");
    };

    assert!(matches!(args.command, ConfigCommand::Path(_)));
}

#[test]
fn base_url_defaults_and_overrides() {
    let cli = Cli::parse_from(["tl", "whoami"]);
    let config = RunConfig::from_cli(&cli).unwrap();
    assert_eq!(config.base_url.as_str(), format!("{DEFAULT_BASE_URL}/"));

    let cli = Cli::parse_from(["tl", "--base-url", TEST_BASE_URL, "whoami"]);
    let config = RunConfig::from_cli(&cli).unwrap();
    assert_eq!(config.base_url.as_str(), format!("{TEST_BASE_URL}/"));
}

#[test]
fn error_kind_exit_codes_match_contract() {
    assert_eq!(ErrorKind::Unexpected.exit_code(), 1);
    assert_eq!(ErrorKind::NetworkFailure.exit_code(), 1);
    assert_eq!(ErrorKind::NotImplemented.exit_code(), 1);
    assert_eq!(ErrorKind::InvalidInput.exit_code(), 2);
    assert_eq!(ErrorKind::AuthenticationRequired.exit_code(), 3);
    assert_eq!(ErrorKind::LoginFailed.exit_code(), 3);
    assert_eq!(ErrorKind::BrowserChallengeRequired.exit_code(), 3);
    assert_eq!(ErrorKind::ParseFailure.exit_code(), 4);
    assert_eq!(ErrorKind::OutputConflict.exit_code(), 5);
}

#[test]
fn shared_models_serialize_with_stable_field_names() {
    let download_url =
        Url::parse("https://www.torrentleech.org/download/1/example.torrent").unwrap();
    let response = SearchResponse {
        query: None,
        page: 1,
        total: Some(1),
        results: vec![SearchResult {
            id: 1,
            title: "example".to_string(),
            category_id: Some(33),
            category: Some("0-day".to_string()),
            tags: vec!["FREELEECH".to_string()],
            added: Some("2026-05-29".to_string()),
            comments: Some(2),
            size: Some("1 MiB".to_string()),
            completed: Some(3),
            seeders: Some(4),
            leechers: Some(5),
            uploader: Some("uploader".to_string()),
            download_url,
            freeleech: true,
        }],
    };

    let json = serde_json::to_value(response).unwrap();
    assert_eq!(json["query"], serde_json::Value::Null);
    assert_eq!(json["results"][0]["category_id"], 33);
    assert_eq!(
        json["results"][0]["download_url"],
        "https://www.torrentleech.org/download/1/example.torrent"
    );

    let download = DownloadInfo {
        id: 1,
        filename: "example.torrent".to_string(),
        source_url: Url::parse("https://www.torrentleech.org/torrent/1").unwrap(),
        saved_path: None,
        bytes: None,
    };
    let category = CategoryInfo {
        id: 33,
        name: "0-day".to_string(),
        group: "Movies".to_string(),
        aliases: vec!["0day".to_string()],
    };
    let details = TorrentDetails {
        id: 1,
        title: "example".to_string(),
        category: Some("0-day".to_string()),
        added: None,
        size: Some("1 MiB".to_string()),
        seeders: Some(4),
        leechers: Some(5),
        completed: Some(3),
        uploader: None,
        tags: vec!["FREELEECH".to_string()],
        description: Some("description".to_string()),
        nfo: None,
        download_url: Url::parse("https://www.torrentleech.org/download/1/example.torrent")
            .unwrap(),
    };

    assert_eq!(
        serde_json::to_value(download).unwrap()["filename"],
        "example.torrent"
    );
    assert_eq!(
        serde_json::to_value(category).unwrap()["aliases"][0],
        "0day"
    );
    assert_eq!(
        serde_json::to_value(details).unwrap()["description"],
        "description"
    );
}
