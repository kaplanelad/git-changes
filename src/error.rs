use std::io;

/// Custom error type for the library
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Git command failed: {0}")]
    GitCommandError(String),

    #[error("Failed to create temporary directory: {0}")]
    TempDirError(String),
}

/// Type alias for Result using the custom Error type
pub type Result<T> = std::result::Result<T, Error>;
