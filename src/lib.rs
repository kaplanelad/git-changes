pub use error::{Error, Result};
pub use types::{FileChange, FileStatus};

mod error;
mod git;
pub mod processor;
mod types;

/// Top-level factory function to create a `GitChangesProcessor`.
/// It determines whether the source is a local path or a remote URL and calls the appropriate constructor.
///
/// # Errors    
///
/// when could not create processor.
pub fn new(repo: &str) -> Result<processor::GitChangesProcessor> {
    processor::GitChangesProcessor::new(repo)
}
