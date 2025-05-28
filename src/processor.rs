use crate::error::Result;
use crate::git::{Git, GitCli};
use crate::FileChange;
use crate::FileStatus;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, instrument};

/// Processes Git repository changes and manages output
pub struct GitChangesProcessor {
    git: GitCli,
}

impl GitChangesProcessor {
    /// Create a processor from a local git repository
    #[instrument(skip(path), fields(path = %path.display()))]
    pub fn new_from_local(path: &Path) -> Result<Self> {
        debug!("Initializing GitChangesProcessor from local repository path");

        Ok(Self {
            git: GitCli::new(path.to_path_buf()),
        })
    }

    /// Creates a new `GitChangesProcessor` from a repository source
    /// The source can be either a local path or a remote URL (HTTPS/SSH)
    ///
    /// # Errors
    ///
    /// Returns an error if the repository cannot be accessed or initialized
    #[instrument(skip(), fields(repo = %repo))]
    pub fn new(repo: &str) -> Result<Self> {
        debug!("Initializing GitChangesProcessor from repository source");

        // Check if source is a URL (simple check for now)
        let is_url = repo.starts_with("https://") || repo.starts_with("git@");

        if is_url {
            debug!("Detected remote repository URL, proceeding with clone operation");
            let git = GitCli::new_with_temp_workspace()?;
            git.clone_repo(repo)?;
            debug!("Successfully cloned remote repository");

            Ok(Self { git })
        } else {
            debug!("Detected local repository path, initializing from local filesystem");
            Self::new_from_local(Path::new(repo))
        }
    }

    /// Exports changes between a branch and the default branch to the specified output directory
    ///
    /// # Errors
    ///
    /// Returns an error if the changes cannot be exported or the output directory is not accessible
    #[instrument(skip(self, output_dir), fields(branch = %branch, output_dir = %output_dir.display()))]
    pub fn export_changes_from_default_branch(
        &self,
        branch: &str,
        output_dir: &Path,
    ) -> Result<HashMap<String, FileChange>> {
        debug!("Exporting changes from branch to default branch");
        let target_branch = self.git.discover_default_branch()?;
        debug!(target_branch = %target_branch, "Discovered default branch, proceeding with export");
        self.export_branch_changes(branch, &target_branch, output_dir)
    }

    /// Exports changes between two branches to the specified output directory
    ///
    /// # Errors
    ///
    /// Returns an error if the changes cannot be exported or the output directory is not accessible
    #[instrument(skip(self, output_dir), fields(branch = %branch, target_branch = %target_branch, output_dir = %output_dir.display()))]
    pub fn export_branch_changes(
        &self,
        branch: &str,
        target_branch: &str,
        output_dir: &Path,
    ) -> Result<HashMap<String, FileChange>> {
        debug!("Starting export of changes between branches");
        self.git.checkout_branch(branch)?;
        let change_files = self.get_changes_for_branch(branch, target_branch)?;
        debug!(
            num_files = change_files.len(),
            "Retrieved changes for branch comparison"
        );

        for (path_str, file_status) in &change_files {
            let output_file_path = output_dir.join(path_str);
            debug!(
                file_path = %path_str,
                status = ?file_status.status,
                "Processing file change"
            );
            match file_status.status {
                FileStatus::Added => {
                    self.git.run_git_command_to_file(
                        &["show", &format!("{branch}:{path_str}")],
                        &output_file_path,
                    )?;
                }
                FileStatus::Modified => {
                    self.git.run_git_command_to_file(
                        &["show", &format!("{branch}:{path_str}")],
                        &output_file_path,
                    )?;
                    let diff_file_path = output_dir.join(format!("{path_str}.diff"));
                    self.git.run_git_command_to_file(
                        &[
                            "diff",
                            &format!("{target_branch}...{branch}"),
                            "--",
                            path_str,
                        ],
                        &diff_file_path,
                    )?;
                }
                FileStatus::Deleted => {
                    debug!("Skipping export of deleted file");
                }
            }
        }
        debug!(
            num_files = change_files.len(),
            "Completed export of all file changes"
        );
        Ok(change_files)
    }

    /// Lists changes between two branches
    ///
    /// # Errors
    ///
    /// Returns an error if the branch changes cannot be retrieved
    #[instrument(skip(self), fields(branch = %branch, target_branch = %target_branch))]
    pub fn list_branch_changes(
        &self,
        branch: &str,
        target_branch: &str,
    ) -> Result<HashMap<String, FileChange>> {
        debug!("Listing changes between branches");
        self.git.checkout_branch(branch)?;
        self.get_changes_for_branch(branch, target_branch)
    }

    /// Lists changes from a branch to the default branch
    ///
    /// # Errors
    ///
    /// Returns an error if the branch changes cannot be retrieved
    #[instrument(skip(self), fields(branch = %branch))]
    pub fn list_changes_from_default_branch(
        &self,
        branch: &str,
    ) -> Result<HashMap<String, FileChange>> {
        debug!("Listing changes from branch to default branch");
        self.git.checkout_branch(branch)?;
        let target_branch = self.git.discover_default_branch()?;
        debug!(target_branch = %target_branch, "Discovered default branch, proceeding with change list");
        self.get_changes_for_branch(branch, &target_branch)
    }

    #[instrument(skip(self), fields(branch_name = %branch_name, target_branch = %target_branch))]
    fn get_changes_for_branch(
        &self,
        branch_name: &str,
        target_branch: &str,
    ) -> Result<HashMap<String, FileChange>> {
        debug!("Retrieving changes between branches");

        let output = self.git.run_git_command(&[
            "diff",
            "--name-status",
            "--no-renames",
            &format!("{target_branch}...{branch_name}"),
        ])?;

        let mut changes = HashMap::with_capacity(output.lines().count());
        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let status_str = parts[0];
                let path_str = parts[1];
                let file_status = match status_str {
                    "A" => FileStatus::Added,
                    "D" => FileStatus::Deleted,
                    _ => FileStatus::Modified,
                };

                debug!(
                    file_path = %path_str,
                    status = ?file_status,
                    "Processing file change from diff"
                );

                changes.insert(
                    path_str.to_string(),
                    FileChange {
                        path: path_str.to_string(),
                        status: file_status,
                    },
                );
            }
        }
        debug!(
            num_changes = changes.len(),
            "Completed processing all file changes"
        );
        Ok(changes)
    }

    /// Lists changes in a specific commit
    ///
    /// # Errors
    ///
    /// Returns an error if the commit changes cannot be retrieved
    #[instrument(skip(self), fields(commit_hash = %commit_hash))]
    pub fn list_commit_changes(&self, commit_hash: &str) -> Result<HashMap<String, FileChange>> {
        debug!("Listing changes for specific commit");
        self.get_changes_for_commits(commit_hash)
    }

    /// Exports changes from a specific commit to the specified output directory
    ///
    /// # Errors
    ///
    /// Returns an error if the changes cannot be exported or the output directory is not accessible
    #[instrument(skip(self, output_dir), fields(commit_hash = %commit_hash, output_dir = %output_dir.display()))]
    pub fn export_commit_changes(
        &self,
        commit_hash: &str,
        output_dir: &Path,
    ) -> Result<HashMap<String, FileChange>> {
        debug!("Starting export of commit changes");
        let change_files = self.get_changes_for_commits(commit_hash)?;
        debug!(
            num_files = change_files.len(),
            "Retrieved changes for commit"
        );

        for (path_str, file_status) in &change_files {
            let output_file_path = output_dir.join(path_str);
            debug!(
                file_path = %path_str,
                status = ?file_status.status,
                "Processing file change from commit"
            );
            match file_status.status {
                FileStatus::Added => {
                    self.git.run_git_command_to_file(
                        &["show", &format!("{commit_hash}:{path_str}")],
                        &output_file_path,
                    )?;
                }
                FileStatus::Modified => {
                    self.git.run_git_command_to_file(
                        &["show", &format!("{commit_hash}:{path_str}")],
                        &output_file_path,
                    )?;
                    let diff_file_path = output_dir.join(format!("{path_str}.diff"));
                    self.git.run_git_command_to_file(
                        &[
                            "diff",
                            &format!("{commit_hash}^..{commit_hash}"),
                            "--",
                            path_str,
                        ],
                        &diff_file_path,
                    )?;
                }
                FileStatus::Deleted => {
                    debug!("Skipping export of deleted file from commit");
                }
            }
        }
        debug!(
            num_files = change_files.len(),
            "Completed export of all commit changes"
        );
        Ok(change_files)
    }

    #[instrument(skip(self), fields(commit_hash = %commit_hash))]
    fn get_changes_for_commits(&self, commit_hash: &str) -> Result<HashMap<String, FileChange>> {
        debug!("Retrieving changes for commit");

        // Check if the commit exists locally first
        let commit_exists = self
            .git
            .run_git_command(&["cat-file", "-e", commit_hash])
            .is_ok();

        // Only fetch if the commit doesn't exist locally
        if !commit_exists {
            debug!("Commit not found locally, attempting to fetch");
            self.git
                .run_git_command(&["fetch", "origin", commit_hash])
                .map_err(|e| {
                    debug!(error = %e, "Failed to fetch commit, will try to use local commit");
                    e
                })?;
        }

        let mut all_changes = HashMap::new();
        let parent_commit = format!("{commit_hash}^");
        debug!(parent_commit = %parent_commit, "Comparing commit with its parent");
        let output = self.git.run_git_command(&[
            "diff",
            "--name-status",
            "--no-renames",
            &parent_commit,
            commit_hash,
        ])?;
        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let status_str = parts[0];
                let path_str = parts[1];
                let file_status = match status_str {
                    "A" => FileStatus::Added,
                    "D" => FileStatus::Deleted,
                    _ => FileStatus::Modified,
                };

                debug!(
                    file_path = %path_str,
                    status = ?file_status,
                    "Processing file change from commit diff"
                );

                all_changes.insert(
                    path_str.to_string(),
                    FileChange {
                        path: path_str.to_string(),
                        status: file_status,
                    },
                );
            }
        }
        debug!(
            num_changes = all_changes.len(),
            "Completed processing all commit changes"
        );
        Ok(all_changes)
    }
}
