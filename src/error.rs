use std::process::ExitCode;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, TlError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Unexpected,
    InvalidInput,
    AuthenticationRequired,
    LoginFailed,
    ParseFailure,
    NetworkFailure,
    OutputConflict,
    BrowserChallengeRequired,
    NotImplemented,
}

impl ErrorKind {
    #[must_use]
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Unexpected | Self::NetworkFailure | Self::NotImplemented => 1,
            Self::InvalidInput => 2,
            Self::AuthenticationRequired | Self::LoginFailed | Self::BrowserChallengeRequired => 3,
            Self::ParseFailure => 4,
            Self::OutputConflict => 5,
        }
    }

    #[must_use]
    pub fn process_exit_code(self) -> ExitCode {
        ExitCode::from(self.exit_code())
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct TlError {
    kind: ErrorKind,
    message: String,
    #[source]
    source: Option<anyhow::Error>,
}

impl TlError {
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    #[must_use]
    pub fn with_source(
        kind: ErrorKind,
        message: impl Into<String>,
        source: impl Into<anyhow::Error>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source.into()),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.kind.exit_code()
    }

    #[must_use]
    pub fn process_exit_code(&self) -> ExitCode {
        self.kind.process_exit_code()
    }
}
