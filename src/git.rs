use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::debug;

use crate::error::{Error, Result};

/// Trait defining Git operations required by the library
pub trait Git {
    /// Clone a Git repository from a URL to a target directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The clone operation fails
    /// - The target directory cannot be created
    /// - The Git command execution fails
    fn clone_repo(&self, url: &str) -> Result<()>;

    /// Get the content of a file at a specific Git reference
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The reference does not exist
    /// - The file does not exist at that reference
    /// - The Git command execution fails
    #[allow(dead_code)] // TODO: Re-evaluate if this method is needed after refactor
    fn get_file_content(&self, ref_name: &str, path: &str) -> Result<Option<String>>;

    /// Run a Git command with the given arguments
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Git command execution fails
    /// - The command output cannot be parsed as UTF-8
    fn run_git_command(&self, args: &[&str]) -> Result<String>;

    /// Run a Git command with the given arguments and stream its stdout to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Git command execution fails
    /// - The output file cannot be created or written to
    fn run_git_command_to_file(&self, args: &[&str], output_file_path: &Path) -> Result<()>;

    /// Checkout a branch
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The branch does not exist
    /// - The Git command execution fails
    fn checkout_branch(&self, branch: &str) -> Result<()>;
}

/// Implementation of Git operations using the local Git CLI
pub struct GitCli {
    repo_path: PathBuf,
    _temp_workspace: Option<tree_fs::Tree>, // To manage lifetime of temp dir
}

impl GitCli {
    /// Creates a new `GitCli` instance with the given repository path
    #[must_use]
    pub const fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            _temp_workspace: None,
        }
    }

    /// Creates a new `GitCli` instance with a temporary workspace
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The temporary directory cannot be created
    pub fn new_with_temp_workspace() -> Result<Self> {
        debug!("Creating temporary workspace");
        let tree = tree_fs::TreeBuilder::default()
            .create()
            .map_err(|e| Error::TempDirError(e.to_string()))?;

        let repo_path = tree.root.clone();

        debug!(
            root = %repo_path.display(),
            "Temporary workspace created"
        );

        Ok(Self {
            repo_path,
            _temp_workspace: Some(tree),
        })
    }
}

impl Git for GitCli {
    fn clone_repo(&self, url: &str) -> Result<()> {
        debug!(url = %url, target_dir = %self.repo_path.display(), "Cloning repository");
        let output = Command::new("git")
            .args(["clone", url, &self.repo_path.to_string_lossy()])
            .current_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
            .output()
            .map_err(|e| Error::GitCommandError(e.to_string()))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            debug!(error = %error, "Clone failed");
            return Err(Error::GitCommandError(error.to_string()));
        }

        debug!(url = %url, target_dir = %self.repo_path.display(), "Repository cloned successfully");
        Ok(())
    }

    #[allow(dead_code)] // Matches trait
    fn get_file_content(&self, ref_name: &str, path: &str) -> Result<Option<String>> {
        debug!(ref_name = %ref_name, path = %path, "Getting file content (dead_code)");
        let output = Command::new("git")
            .args(["show", &format!("{ref_name}:{path}")])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::GitCommandError(e.to_string()))?;

        if !output.status.success() {
            // If git show fails, it might mean the file doesn't exist at this ref, which is not an error for us.
            // An empty string could also mean an empty file.
            // For deleted files, git show <ref>:<path> would fail if path existed in <ref> but not HEAD.
            // If we try to get content of a deleted file from the commit *before* deletion, it's fine.
            // If we try to get content of a deleted file from the commit *of* deletion, it *should* not exist.
            debug!(ref_name = %ref_name, path = %path, status = %output.status, "File not found or command failed for get_file_content");
            return Ok(None);
        }

        let content =
            String::from_utf8(output.stdout).map_err(|e| Error::GitCommandError(e.to_string()))?;

        debug!(ref_name = %ref_name, path = %path, "File content retrieved");
        Ok(Some(content))
    }

    fn run_git_command(&self, args: &[&str]) -> Result<String> {
        debug!(args = ?args, repo_path = %self.repo_path.display(), "Running git command");
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::GitCommandError(e.to_string()))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            debug!(error = %error, "Git command failed");
            return Err(Error::GitCommandError(error.to_string()));
        }

        let result = String::from_utf8(output.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|e| Error::GitCommandError(e.to_string()))?;

        debug!(args = ?args, "Git command completed successfully");
        Ok(result)
    }

    fn run_git_command_to_file(&self, args: &[&str], output_file_path: &Path) -> Result<()> {
        debug!(args = ?args, output_file = %output_file_path.display(), repo_path = %self.repo_path.display(), "Running git command to file");
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::GitCommandError(e.to_string()))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            debug!(error = %error, "Git command to file failed");
            return Err(Error::GitCommandError(error.to_string()));
        }

        // Ensure parent directory exists
        if let Some(parent_dir) = output_file_path.parent() {
            if !parent_dir.exists() {
                std::fs::create_dir_all(parent_dir).map_err(Error::IoError)?;
            }
        }

        let mut file = File::create(output_file_path).map_err(Error::IoError)?;
        file.write_all(&output.stdout).map_err(Error::IoError)?;

        debug!(args = ?args, output_file = %output_file_path.display(), "Git command to file completed successfully");
        Ok(())
    }

    fn checkout_branch(&self, branch: &str) -> Result<()> {
        debug!(branch = %branch, repo_path = %self.repo_path.display(), "Checking out branch");
        let output = Command::new("git")
            .args(["checkout", branch])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::GitCommandError(e.to_string()))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            debug!(error = %error, "Checkout failed");
            return Err(Error::GitCommandError(error.to_string()));
        }

        debug!(branch = %branch, "Branch checked out successfully");
        Ok(())
    }
}
