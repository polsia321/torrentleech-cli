use crate::app::AppContext;
use crate::auth::LoginConfig;
use crate::cli::DownloadArgs;
use crate::client::TlClient;
use crate::commands::search::resolve_credentials;
use crate::config::{EnvConfig, RawConfig, ResolveOptions, ResolvedConfig};
use crate::download::{
    ConflictPolicy, OutputRequest, TargetKind, parse_detail_download_link, persist_torrent,
    resolve_target,
};
use crate::error::Result;

const MAX_TORRENT_BYTES: usize = 20 * 1024 * 1024;

pub fn run(context: &AppContext, args: DownloadArgs) -> Result<()> {
    let target = resolve_target(&args.target, &context.config.base_url)?;
    let resolved = command_config(context, &args)?;
    let output_dir = resolved.output_dir.clone();
    let credentials = resolve_credentials(resolved)?;
    let client = TlClient::new(LoginConfig::new(
        context.config.base_url.clone(),
        credentials.cookie_jar.clone(),
        credentials.credentials,
    ))?;

    let mut detail_filename_hint = None;
    let download_url = match target.kind {
        TargetKind::Detail => {
            let html = client.get_text(target.url.as_str())?;
            let url = parse_detail_download_link(&html, target.id, &context.config.base_url)?;
            detail_filename_hint = download_filename_hint(&url);
            url
        }
        TargetKind::DirectDownload => target.url.clone(),
    };
    let bytes = client.get_bytes_limited(download_url.as_str(), MAX_TORRENT_BYTES)?;
    let output = persist_torrent(
        &OutputRequest {
            output_dir,
            filename: args.filename,
            filename_hint: target.filename_hint.or(detail_filename_hint),
            conflict_policy: if args.force {
                ConflictPolicy::Overwrite
            } else {
                ConflictPolicy::Fail
            },
        },
        &bytes,
    )?;

    if args.print_path {
        println!("{}", output.display());
    }

    Ok(())
}

fn command_config(context: &AppContext, args: &DownloadArgs) -> Result<ResolvedConfig> {
    let raw = if context.config.config_path.exists() {
        RawConfig::load(&context.config.config_path, dirs::home_dir().as_deref())?
    } else {
        RawConfig::default()
    };

    ResolvedConfig::resolve(
        raw,
        &EnvConfig::current(),
        &crate::config::ConfigPaths::defaults(),
        ResolveOptions {
            config_path: Some(context.config.config_path.clone()),
            output_dir: args.output_dir.clone(),
            ..ResolveOptions::default()
        },
    )
}

fn download_filename_hint(url: &url::Url) -> Option<String> {
    url.path_segments()?.next_back().map(ToString::to_string)
}
