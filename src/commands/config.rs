use std::fs;
use std::io;
use std::path::Path;

use crate::app::AppContext;
use crate::cli::{ConfigArgs, ConfigCommand, ConfigInitArgs, ConfigShowArgs};
use crate::config::RawConfig;
use crate::error::{ErrorKind, Result, TlError};

const DEFAULT_CONFIG: &str = r#"output_dir = "."
default_limit = 10
"#;

pub fn run(context: &AppContext, args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Init(args) => init(context, args),
        ConfigCommand::Path(_) => path(context),
        ConfigCommand::Show(args) => show(context, args),
    }
}

fn init(context: &AppContext, args: ConfigInitArgs) -> Result<()> {
    let config_path = args
        .path
        .unwrap_or_else(|| context.config.config_path.clone());
    if config_path.exists() {
        return Err(TlError::new(
            ErrorKind::OutputConflict,
            format!("config file already exists at {}", config_path.display()),
        ));
    }

    if let Some(parent) = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| {
            TlError::with_source(
                ErrorKind::Unexpected,
                format!("failed to create config directory {}", parent.display()),
                source,
            )
        })?;
    }

    write_new_config(&config_path)?;
    println!("{}", config_path.display());
    Ok(())
}

fn path(context: &AppContext) -> Result<()> {
    println!("{}", context.config.config_path.display());
    Ok(())
}

fn show(context: &AppContext, args: ConfigShowArgs) -> Result<()> {
    let raw = RawConfig::load(&context.config.config_path, dirs::home_dir().as_deref())?;
    let display = raw.to_display();

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&display).expect("config display serializes")
        );
    } else if let Some(file) = display.file {
        if let Some(username) = file.username {
            println!("username: {username}");
        }
        if let Some(cookie_jar) = file.cookie_jar {
            println!("cookie_jar: {}", cookie_jar.display());
        }
        if let Some(output_dir) = file.output_dir {
            println!("output_dir: {}", output_dir.display());
        }
        if let Some(default_limit) = file.default_limit {
            println!("default_limit: {default_limit}");
        }
        if let Some(password_file) = file.password_file {
            println!("password_file: {}", password_file.display());
        }
    }

    Ok(())
}

fn write_new_config(path: &Path) -> Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, DEFAULT_CONFIG.as_bytes()))
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                TlError::with_source(
                    ErrorKind::OutputConflict,
                    format!("config file already exists at {}", path.display()),
                    source,
                )
            } else {
                TlError::with_source(
                    ErrorKind::Unexpected,
                    format!("failed to write config file {}", path.display()),
                    source,
                )
            }
        })
}
