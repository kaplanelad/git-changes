use crate::error::Result;
use crate::git::{Git, GitCli};
use crate::types::AnalysisTarget;
use crate::FileChange;
use crate::FileStatus;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{debug, instrument};

/// Processes Git repository changes and manages output
pub struct GitChangesProcessor<'a> {
    git: GitCli,
    target_branch: Option<Cow<'a, str>>,
}

impl<'a> GitChangesProcessor<'a> {
    /// Create a processor from a local git repository
    #[instrument(skip(path, target_branch))]
    pub fn new_from_local(path: &Path, target_branch: Option<&'a str>) -> Result<Self> {
        debug!(
            path = %path.display(),
            target_branch = ?target_branch,
            "Creating processor from local repository"
        );

        Ok(Self {
            git: GitCli::new(path.to_path_buf()),
            target_branch: target_branch.map(Cow::Borrowed),
        })
    }

    /// Create a processor by cloning a remote repository
    #[instrument(skip(target_branch))]
    pub fn new_from_remote(repo_url: &str, target_branch: Option<&'a str>) -> Result<Self> {
        debug!(
            repo_url = %repo_url,
            target_branch = ?target_branch,
            "Creating processor from remote repository"
        );

        let git = GitCli::new_with_temp_workspace()?;
        git.clone_repo(repo_url)?;
        debug!(repo_url = %repo_url, "Repository cloned successfully");

        Ok(Self {
            git,
            target_branch: target_branch.map(Cow::Borrowed),
        })
    }

    /// Creates a new `GitChangesProcessor` from a repository source
    /// The source can be either a local path or a remote URL (HTTPS/SSH)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - For remote URLs: The repository cannot be cloned or the temporary workspace cannot be created
    /// - For local paths: The repository path does not exist
    /// - Git operations fail
    #[instrument(skip(target_branch))]
    pub fn new(repo: &str, target_branch: Option<&'a str>) -> Result<Self> {
        debug!(
            repo = %repo,
            target_branch = ?target_branch,
            "Creating processor"
        );

        // Check if source is a URL (simple check for now)
        let is_url = repo.starts_with("https://") || repo.starts_with("git@");

        if is_url {
            debug!("Repository is a remote URL, creating from remote");
            Self::new_from_remote(repo, target_branch)
        } else {
            debug!("Repository is a local path, creating from local");
            Self::new_from_local(Path::new(repo), target_branch)
        }
    }

    /// Analyze changes in a specific branch
    ///
    /// # Errors
    ///
    /// When could not checkout branch.
    pub fn analyze_branch(&self, branch: &str) -> Result<AnalysisTarget> {
        self.git.checkout_branch(branch)?;
        Ok(AnalysisTarget::Branch(branch.to_string()))
    }

    /// Analyze changes in specific commits
    #[must_use]
    pub fn analyze_commits(&self, commits: &[String]) -> AnalysisTarget {
        AnalysisTarget::Commits(commits.to_vec())
    }

    /// Lists file changes for the given target without writing to disk.
    #[instrument(skip(self))]
    pub fn list_changes(&self, target: &AnalysisTarget) -> Result<HashMap<String, FileChange>> {
        debug!(target = ?target, "Listing changes");
        match target {
            AnalysisTarget::Branch(branch) => self.get_changes_for_branch(branch, false, None),
            AnalysisTarget::Commits(commits) => {
                GitChangesProcessor::get_changes_for_commits(commits, &self.git, false, None)
            }
        }
    }

    /// Exports file changes for the given target to the specified output directory.
    /// This includes copying new/modified files and generating .diff files for modifications.
    #[instrument(skip(self))]
    pub fn export_changes(
        &self,
        target: &AnalysisTarget,
        output_dir: &Path,
    ) -> Result<HashMap<String, FileChange>> {
        debug!(target = ?target, output_dir = %output_dir.display(), "Exporting changes");
        if !output_dir.exists() {
            debug!(path = %output_dir.display(), "Creating base output directory for export");
            fs::create_dir_all(output_dir)?;
        }
        match target {
            AnalysisTarget::Branch(branch) => {
                self.get_changes_for_branch(branch, true, Some(output_dir))
            }
            AnalysisTarget::Commits(commits) => GitChangesProcessor::get_changes_for_commits(
                commits,
                &self.git,
                true,
                Some(output_dir),
            ),
        }
    }

    #[instrument(skip(self))]
    fn get_changes_for_branch(
        &self,
        branch_name: &str,
        apply: bool,
        output_dir_override: Option<&Path>,
    ) -> Result<HashMap<String, FileChange>> {
        debug!(branch = %branch_name, apply = apply, output_dir = ?output_dir_override, "Getting changes for branch");

        let current_output_dir = if apply {
            output_dir_override
                .expect("output_dir_override must be Some when apply is true for branch changes")
        } else {
            Path::new("")
        };

        let target_branch_cow = self.target_branch.as_ref().map_or_else(
            || {
                Cow::Owned(
                    self.git
                        .run_git_command(&["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
                        .unwrap_or_else(|_| "origin/main".to_string()),
                )
            },
            |branch_ref| Cow::Borrowed(branch_ref.as_ref()),
        );
        let target_branch = target_branch_cow.as_ref();

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

                if apply {
                    let output_file_path = current_output_dir.join(path_str);
                    match file_status {
                        FileStatus::Added => {
                            self.git.run_git_command_to_file(
                                &["show", &format!("{branch_name}:{path_str}")],
                                &output_file_path,
                            )?;
                        }
                        FileStatus::Modified => {
                            self.git.run_git_command_to_file(
                                &["show", &format!("{branch_name}:{path_str}")],
                                &output_file_path,
                            )?;
                            let diff_file_path =
                                current_output_dir.join(format!("{path_str}.diff"));
                            self.git.run_git_command_to_file(
                                &[
                                    "diff",
                                    &format!("{target_branch}...{branch_name}"),
                                    "--",
                                    path_str,
                                ],
                                &diff_file_path,
                            )?;
                        }
                        FileStatus::Deleted => {}
                    }
                }
                changes.insert(
                    path_str.to_string(),
                    FileChange {
                        path: path_str.to_string(),
                        status: file_status,
                    },
                );
            }
        }
        Ok(changes)
    }

    #[instrument(skip(git))]
    fn get_changes_for_commits(
        commit_hashes: &[String],
        git: &impl Git,
        apply: bool,
        output_dir_override: Option<&Path>,
    ) -> Result<HashMap<String, FileChange>> {
        debug!(
            commits_count = commit_hashes.len(),
            apply = apply,
            output_dir = ?output_dir_override,
            "Getting changes for commits"
        );
        let current_output_dir = if apply {
            output_dir_override
                .expect("output_dir_override must be Some when apply is true for commit changes")
        } else {
            Path::new("")
        };

        let mut all_changes = HashMap::with_capacity(commit_hashes.len() * 5);
        for commit_hash in commit_hashes {
            let parent_commit = format!("{commit_hash}^");
            let output = git.run_git_command(&[
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

                    if apply {
                        let output_file_path = current_output_dir.join(path_str);
                        match file_status {
                            FileStatus::Added => {
                                git.run_git_command_to_file(
                                    &["show", &format!("{commit_hash}:{path_str}")],
                                    &output_file_path,
                                )?;
                            }
                            FileStatus::Modified => {
                                git.run_git_command_to_file(
                                    &["show", &format!("{commit_hash}:{path_str}")],
                                    &output_file_path,
                                )?;
                                let diff_file_path =
                                    current_output_dir.join(format!("{path_str}.diff"));
                                git.run_git_command_to_file(
                                    &[
                                        "diff",
                                        &format!("{parent_commit}..{commit_hash}"),
                                        "--",
                                        path_str,
                                    ],
                                    &diff_file_path,
                                )?;
                            }
                            FileStatus::Deleted => {}
                        }
                    }
                    all_changes.insert(
                        path_str.to_string(),
                        FileChange {
                            path: path_str.to_string(),
                            status: file_status,
                        },
                    );
                }
            }
        }
        Ok(all_changes)
    }
}
