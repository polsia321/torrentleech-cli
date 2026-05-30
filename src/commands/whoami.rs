use crate::app::AppContext;
use crate::auth::LoginConfig;
use crate::cli::WhoamiArgs;
use crate::client::TlClient;
use crate::commands::search::resolve_credentials;
use crate::config::{ConfigPaths, EnvConfig, ResolveOptions, ResolvedConfig};
use crate::error::{ErrorKind, Result, TlError};

pub fn run(context: &AppContext, args: WhoamiArgs) -> Result<()> {
    let raw = super::login::load_raw_config(&context.config.config_path)?;
    let resolved = ResolvedConfig::resolve(
        raw,
        &EnvConfig::current(),
        &ConfigPaths::defaults(),
        ResolveOptions {
            config_path: Some(context.config.config_path.clone()),
            cookie_jar: args.cookie_jar,
            ..ResolveOptions::default()
        },
    )?;
    let credentials = resolve_credentials(resolved)?;
    let client = TlClient::new(LoginConfig::new(
        context.config.base_url.clone(),
        credentials.cookie_jar,
        credentials.credentials,
    ))?;
    let html = client.get_text("/")?;
    let username = parse_username(&html)?;

    println!("username: {username}");
    Ok(())
}

fn parse_username(html: &str) -> Result<String> {
    let document = scraper::Html::parse_document(html);
    for selector in ["header [data-username]", "nav [data-username]"] {
        let selector = username_selector(selector)?;
        for element in document.select(&selector) {
            if let Some(username) = element
                .value()
                .attr("data-username")
                .and_then(clean_username)
            {
                return Ok(username);
            }
        }
    }
    for selector in [
        "header a[href*='/user/']",
        "nav a[href*='/user/']",
        "[onclick*='/profile/']",
    ] {
        let selector = username_selector(selector)?;
        for element in document.select(&selector) {
            if let Some(href) = element.value().attr("href")
                && is_account_href(href)
            {
                continue;
            }
            let text = element.text().collect::<String>();
            if let Some(username) = clean_username(&text) {
                return Ok(username);
            }
        }
    }

    Err(TlError::new(
        ErrorKind::ParseFailure,
        "username was not found in authenticated page",
    ))
}

fn username_selector(selector: &str) -> Result<scraper::Selector> {
    scraper::Selector::parse(selector).map_err(|error| {
        TlError::new(
            ErrorKind::ParseFailure,
            format!("username selector is invalid: {error}"),
        )
    })
}

fn is_account_href(href: &str) -> bool {
    href.trim_start_matches('/').starts_with("user/account/")
}

fn clean_username(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("logout") {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_username;

    #[test]
    fn parses_username_from_header_navigation() {
        let html = r#"<header><nav><a href="/user/example/profile/">example</a></nav></header>"#;

        assert_eq!(parse_username(html).unwrap(), "example");
    }

    #[test]
    fn skips_account_links_before_username_links() {
        let html = r#"<header><a href="/user/account/logout/">Log out</a><a href="/user/example/profile/">example</a></header>"#;

        assert_eq!(parse_username(html).unwrap(), "example");
    }

    #[test]
    fn parses_username_from_data_attribute() {
        let html = r#"<nav><span data-username="example">ignored</span></nav>"#;

        assert_eq!(parse_username(html).unwrap(), "example");
    }

    #[test]
    fn parses_username_from_profile_onclick() {
        let html = r#"<span onclick="window.location.href='/profile/example/view'"><span class="user_poweruser">example</span></span>"#;

        assert_eq!(parse_username(html).unwrap(), "example");
    }
}
