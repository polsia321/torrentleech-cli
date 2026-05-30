use std::io;

use crate::app::AppContext;
use crate::auth::{Credentials, LoginConfig};
use crate::cli::LoginArgs;
use crate::client::TlClient;
use crate::config::{
    ConfigPaths, CredentialPrompt, CredentialResolver, EnvConfig, LoginOverrides, RawConfig,
    ResolveOptions, ResolvedConfig,
};
use crate::error::{ErrorKind, Result, TlError};

pub fn run(context: &AppContext, args: LoginArgs) -> Result<()> {
    let resolved = resolve_login_config(context, &args)?;
    let username = resolved.username.clone().ok_or_else(|| {
        TlError::new(
            ErrorKind::AuthenticationRequired,
            "username required to authenticate",
        )
    })?;
    let mut stdin = io::stdin().lock();
    let mut prompt = CredentialPrompt::tty();
    let password = CredentialResolver::new(
        &resolved,
        LoginOverrides {
            password_file: args.password_file,
            password_stdin: args.password_stdin,
        },
    )
    .read_password(&mut stdin, &mut prompt)?;
    let cookie_jar = resolved.cookie_jar.clone();
    let client = TlClient::new(LoginConfig::new(
        context.config.base_url.clone(),
        resolved.cookie_jar,
        Some(Credentials::new(username, password)),
    ))?;

    client.login()?;
    println!("saved session to {}", cookie_jar.display());
    Ok(())
}

pub(crate) fn resolve_login_config(
    context: &AppContext,
    args: &LoginArgs,
) -> Result<ResolvedConfig> {
    let raw = load_raw_config(&context.config.config_path)?;
    ResolvedConfig::resolve(
        raw,
        &EnvConfig::current(),
        &ConfigPaths::defaults(),
        ResolveOptions {
            config_path: Some(context.config.config_path.clone()),
            username: args.username.clone(),
            cookie_jar: args.cookie_jar.clone(),
            password_file: args.password_file.clone(),
            password_stdin: args.password_stdin,
            ..ResolveOptions::default()
        },
    )
}

pub(crate) fn load_raw_config(path: &std::path::Path) -> Result<RawConfig> {
    if path.exists() {
        RawConfig::load(path, dirs::home_dir().as_deref())
    } else {
        Ok(RawConfig::default())
    }
}
