use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, instrument};

use crate::error::{Error, Result};

/// Trait defining Git operations required by the library
pub trait Git {
    /// Clone a Git repository from a URL to a target directory
    ///
    /// # Errors
    ///
    /// Returns an error if the repository cannot be cloned
    fn clone_repo(&self, url: &str) -> Result<()>;

    /// Get the content of a file at a specific Git reference
    ///
    /// # Errors
    ///
    /// Returns an error if the file content cannot be retrieved
    #[allow(dead_code)] // TODO: Re-evaluate if this method is needed after refactor
    fn get_file_content(&self, ref_name: &str, path: &str) -> Result<Option<String>>;

    /// Run a Git command with the given arguments
    ///
    /// # Errors
    ///
    /// Returns an error if the git command cannot be executed
    fn run_git_command(&self, args: &[&str]) -> Result<String>;

    /// Run a Git command with the given arguments and stream its stdout to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the command output cannot be written to the file
    fn run_git_command_to_file(&self, args: &[&str], output_file_path: &Path) -> Result<()>;

    /// Checkout a branch
    ///
    /// # Errors
    ///
    /// Returns an error if the branch cannot be checked out
    fn checkout_branch(&self, branch: &str) -> Result<()>;

    /// Discover the default branch of the remote repository (e.g., origin/main or origin/master)
    ///
    /// # Errors
    ///
    /// Returns an error if the default branch cannot be discovered
    fn discover_default_branch(&self) -> Result<String>;
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
    /// Returns an error if the temporary workspace cannot be created
    pub fn new_with_temp_workspace() -> Result<Self> {
        debug!("Creating temporary workspace");
        let tree = tree_fs::TreeBuilder::default()
            .create()
            .map_err(|e| Error::TempDirError(e.to_string()))?;

        let repo_path = tree.root.clone();

        debug!("Temporary workspace created");

        Ok(Self {
            repo_path,
            _temp_workspace: Some(tree),
        })
    }
}

impl Git for GitCli {
    #[instrument(skip(self), fields(url = %url, target_dir = %self.repo_path.display()))]
    fn clone_repo(&self, url: &str) -> Result<()> {
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

        debug!("Repository cloned successfully");
        Ok(())
    }

    #[allow(dead_code)] // Matches trait
    #[instrument(skip(self), fields(ref_name = %ref_name, path = %path))]
    fn get_file_content(&self, ref_name: &str, path: &str) -> Result<Option<String>> {
        let output = Command::new("git")
            .args(["show", &format!("{ref_name}:{path}")])
            .current_dir(&self.repo_path)
            .output()
            .map_err(|e| Error::GitCommandError(e.to_string()))?;

        if !output.status.success() {
            debug!(status = %output.status, "File not found or command failed");
            return Ok(None);
        }

        let content =
            String::from_utf8(output.stdout).map_err(|e| Error::GitCommandError(e.to_string()))?;

        debug!(content_length = content.len(), "File content retrieved");
        Ok(Some(content))
    }

    #[instrument(skip(self), fields(args = ?args, repo_path = %self.repo_path.display()))]
    fn run_git_command(&self, args: &[&str]) -> Result<String> {
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

        debug!(
            output_length = result.len(),
            "Git command completed successfully"
        );
        Ok(result)
    }

    #[instrument(skip(self), fields(args = ?args, output_file = %output_file_path.display(), repo_path = %self.repo_path.display()))]
    fn run_git_command_to_file(&self, args: &[&str], output_file_path: &Path) -> Result<()> {
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
                debug!(parent_dir = %parent_dir.display(), "Creating parent directory");
                std::fs::create_dir_all(parent_dir).map_err(Error::IoError)?;
            }
        }

        let mut file = File::create(output_file_path).map_err(Error::IoError)?;
        file.write_all(&output.stdout).map_err(Error::IoError)?;

        debug!(
            output_size = output.stdout.len(),
            "Git command to file completed successfully"
        );
        Ok(())
    }

    #[instrument(skip(self), fields(branch = %branch, repo_path = %self.repo_path.display()))]
    fn checkout_branch(&self, branch: &str) -> Result<()> {
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

        debug!("Branch checked out successfully");
        Ok(())
    }

    #[instrument(skip(self), fields(repo_path = %self.repo_path.display()))]
    fn discover_default_branch(&self) -> Result<String> {
        let branch_name =
            self.run_git_command(&["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])?;
        debug!(default_branch = %branch_name, "Default branch discovered");
        Ok(branch_name)
    }
}
