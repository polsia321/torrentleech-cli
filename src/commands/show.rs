use crate::app::AppContext;
use crate::auth::LoginConfig;
use crate::cli::ShowArgs;
use crate::client::TlClient;
use crate::commands::search::resolve_credentials;
use crate::config::{ConfigPaths, EnvConfig, RawConfig, ResolveOptions, ResolvedConfig};
use crate::download::{TargetKind, resolve_target};
use crate::error::Result;
use crate::output::{render_show_compact, render_show_description, render_show_json};
use crate::show::parse_torrent_details;

pub fn run(context: &AppContext, args: ShowArgs) -> Result<()> {
    let target = resolve_target(&args.target, &context.config.base_url)?;
    let detail_url = match target.kind {
        TargetKind::Detail => target.url,
        TargetKind::DirectDownload => context
            .config
            .base_url
            .join(&format!("/torrent/{}", target.id))
            .map_err(|error| {
                crate::error::TlError::with_source(
                    crate::error::ErrorKind::InvalidInput,
                    "invalid base URL",
                    error,
                )
            })?,
    };
    let resolved = command_config(context)?;
    let credentials = resolve_credentials(resolved)?;
    let client = TlClient::new(LoginConfig::new(
        context.config.base_url.clone(),
        credentials.cookie_jar.clone(),
        credentials.credentials,
    ))?;

    let html = client.get_text(detail_url.as_str())?;
    let details = parse_torrent_details(&html, detail_url.as_ref(), &context.config.base_url)?;

    if args.json {
        println!("{}", render_show_json(&details)?);
    } else if args.description_only {
        print!("{}", render_show_description(&details));
    } else {
        print!("{}", render_show_compact(&details));
    }

    Ok(())
}

fn command_config(context: &AppContext) -> Result<ResolvedConfig> {
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
