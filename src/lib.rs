pub use error::{Error, Result};
pub use types::{AnalysisTarget, FileChange, FileStatus};

use std::path::Path;

mod error;
mod git;
pub mod processor;
mod types;

/// Creates a new `GitChangesProcessor` for a local repository.
///
/// # Arguments
///
/// * `repo_path`: The path to the local Git repository.
/// * `target_branch`: Optional target branch to compare against. Defaults to `origin/HEAD` or `origin/main`.
///
/// # Errors
///
/// Returns an error if the repository path does not exist or if Git operations fail.
pub fn new_from_local<'a>(
    repo_path: &Path,
    target_branch: Option<&'a str>,
) -> Result<processor::GitChangesProcessor<'a>> {
    processor::GitChangesProcessor::new_from_local(repo_path, target_branch)
}

/// Creates a new `GitChangesProcessor` by cloning a remote repository.
///
/// # Arguments
///
/// * `repo_url`: The URL of the remote Git repository (HTTPS or SSH).
/// * `target_branch`: Optional target branch to compare against. Defaults to `origin/HEAD` or `origin/main`.
///
/// # Errors
///
/// Returns an error if cloning fails or if Git operations fail.
pub fn new_from_remote<'a>(
    repo_url: &str,
    target_branch: Option<&'a str>,
) -> Result<processor::GitChangesProcessor<'a>> {
    processor::GitChangesProcessor::new_from_remote(repo_url, target_branch)
}

/// Top-level factory function to create a `GitChangesProcessor`.
/// It determines whether the source is a local path or a remote URL and calls the appropriate constructor.
///
/// # Errors    
///
/// when could not create processor.
pub fn new<'a>(
    repo: &'a str,
    target_branch: Option<&'a str>,
) -> Result<processor::GitChangesProcessor<'a>> {
    processor::GitChangesProcessor::new(repo, target_branch)
}
