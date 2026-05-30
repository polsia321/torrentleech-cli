use std::fs;

use crate::app::AppContext;
use crate::cli::LogoutArgs;
use crate::config::{ConfigPaths, EnvConfig, ResolveOptions, ResolvedConfig};
use crate::error::{ErrorKind, Result, TlError};

pub fn run(context: &AppContext, args: LogoutArgs) -> Result<()> {
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

    match fs::remove_file(&resolved.cookie_jar) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(TlError::with_source(
            ErrorKind::Unexpected,
            format!(
                "failed to remove cookie jar {}",
                resolved.cookie_jar.display()
            ),
            source,
        )),
    }
}
