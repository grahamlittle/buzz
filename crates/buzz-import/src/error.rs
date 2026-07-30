//! Error type and process exit codes.
//!
//! Exit codes mirror the `buzz-cli` contract so the importer composes with the
//! same tooling: `0` ok, `1` input error, `2` network/relay, `3` auth,
//! `4` other, `5` write conflict (NIP-33 LWW).

use thiserror::Error;

/// Process exit codes, matching the `buzz-cli` convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Ok = 0,
    Input = 1,
    Network = 2,
    Auth = 3,
    Other = 4,
    WriteConflict = 5,
}

/// Top-level importer error.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("input error: {0}")]
    Input(String),

    #[error("network/relay error: {0}")]
    Network(String),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("write conflict (NIP-33 LWW): {0}")]
    WriteConflict(String),

    #[error("{0}")]
    Other(String),
}

impl ImportError {
    /// Map the error to its process exit code.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            ImportError::Input(_) => ExitCode::Input,
            ImportError::Network(_) => ExitCode::Network,
            ImportError::Auth(_) => ExitCode::Auth,
            ImportError::WriteConflict(_) => ExitCode::WriteConflict,
            ImportError::Other(_) => ExitCode::Other,
        }
    }
}

/// Crate result alias.
pub type Result<T> = std::result::Result<T, ImportError>;
