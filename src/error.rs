#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Git command failed: {0}")]
    GitCommandError(String),

    #[error("Failed to create temporary directory: {0}")]
    TempDirError(String),
}

pub type Result<T> = std::result::Result<T, Error>;
