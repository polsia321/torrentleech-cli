use time::OffsetDateTime;

use crate::app::AppContext;
use crate::auth::{Credentials, LoginConfig};
use crate::categories::parse_selection;
use crate::cli::SearchArgs;
use crate::client::TlClient;
use crate::config::{
    ConfigPaths, CredentialPrompt, CredentialResolver, EnvConfig, LoginOverrides, RawConfig,
    ResolveOptions, ResolvedConfig,
};
use crate::error::{ErrorKind, Result, TlError};
use crate::model::SearchResponse;
use crate::output::{render_search_compact, render_search_json};
use crate::search::{SearchRequest, build_search_list_url, parse_browse_json};

const DEFAULT_LIMIT: u32 = 10;
const MAX_LIMIT: u32 = 100;

pub fn run(context: &AppContext, args: SearchArgs) -> Result<()> {
    let resolved = resolve_config(context)?;
    let limit = resolve_limit(args.limit, &resolved)?;
    let categories = resolve_categories(&args.categories)?;
    let credentials = resolve_credentials(resolved)?;
    let client = TlClient::new(LoginConfig::new(
        context.config.base_url.clone(),
        credentials.cookie_jar.clone(),
        credentials.credentials,
    ))?;
    let response = fetch_search(context, &client, &args, categories, limit)?;

    if args.json {
        println!("{}", render_search_json(&response)?);
    } else {
        print!(
            "{}",
            render_search_compact(&response, OffsetDateTime::now_utc())?
        );
    }

    Ok(())
}

fn fetch_search(
    context: &AppContext,
    client: &TlClient,
    args: &SearchArgs,
    categories: Vec<u32>,
    limit: u32,
) -> Result<SearchResponse> {
    let mut page = args.page;
    let mut total = None;
    let mut results = Vec::new();

    loop {
        let request = SearchRequest {
            query: args.query.clone(),
            categories: categories.clone(),
            freeleech: args.freeleech,
            page,
            sort: args.sort,
            order: args.order,
        };
        let url = build_search_list_url(&context.config.base_url, &request)?;
        let body = client.get_text(url.as_str())?;
        let browse_page = parse_browse_json(&body, &context.config.base_url)?;
        if total.is_none() {
            total = browse_page.total;
        }
        results.extend(browse_page.results);

        if results.len() >= limit as usize || !browse_page.has_next_page {
            break;
        }
        page = page
            .checked_add(1)
            .ok_or_else(|| TlError::new(ErrorKind::InvalidInput, "page number is too large"))?;
    }

    results.truncate(limit as usize);

    Ok(SearchResponse {
        query: args.query.clone(),
        page: args.page,
        total,
        results,
    })
}

fn resolve_limit(flag_limit: Option<u32>, resolved: &ResolvedConfig) -> Result<u32> {
    let limit = flag_limit
        .or(resolved.default_limit)
        .unwrap_or(DEFAULT_LIMIT);

    if limit > MAX_LIMIT {
        return Err(TlError::new(
            ErrorKind::InvalidInput,
            format!("limit must be at most {MAX_LIMIT}"),
        ));
    }

    Ok(limit)
}

fn resolve_categories(inputs: &[String]) -> Result<Vec<u32>> {
    let mut category_ids = Vec::new();
    for input in inputs {
        category_ids.extend(parse_selection(input)?.category_ids());
    }
    Ok(category_ids)
}

pub(crate) fn resolve_credentials(resolved: ResolvedConfig) -> Result<SearchCredentials> {
    let credentials = match resolved.username.clone() {
        Some(username) => {
            let mut stdin = std::io::stdin().lock();
            let mut prompt = CredentialPrompt::disabled();
            CredentialResolver::new(&resolved, LoginOverrides::default())
                .read_password(&mut stdin, &mut prompt)
                .ok()
                .map(|password| Credentials::new(username, password))
        }
        None => None,
    };

    Ok(SearchCredentials {
        cookie_jar: resolved.cookie_jar,
        credentials,
    })
}

fn resolve_config(context: &AppContext) -> Result<ResolvedConfig> {
    let raw = if context.config.config_path.exists() {
        RawConfig::load(&context.config.config_path, dirs::home_dir().as_deref())?
    } else {
        RawConfig::default()
    };

    ResolvedConfig::resolve(
        raw,
        &EnvConfig::current(),
        &ConfigPaths::defaults(),
        ResolveOptions {
            config_path: Some(context.config.config_path.clone()),
            ..ResolveOptions::default()
        },
    )
}

pub(crate) struct SearchCredentials {
    pub(crate) cookie_jar: std::path::PathBuf,
    pub(crate) credentials: Option<Credentials>,
}
