use git_changes::{self, Error, FileStatus, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use tree_fs::{Tree, TreeBuilder};

fn setup_test_repo() -> (Tree, Tree) {
    // Create a temporary workspace for the test repository
    let tree = TreeBuilder::default()
        .add_file("file1.txt", "original content")
        .add_file("dir1/file2.txt", "file 2 content")
        .create()
        .expect("Failed to create test repo tree");

    // Create another temporary directory for the output
    let output_tree = TreeBuilder::default()
        .create()
        .expect("Failed to create output tree");

    // Initialize git repository
    Command::new("git")
        .args(["init"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to init git repo");

    // Configure git user for commits
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to set git user name");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to set git user email");

    // Add and commit initial files
    Command::new("git")
        .args(["add", "."])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to add initial files");
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to commit initial files");

    // Create main branch and set as default
    Command::new("git")
        .args(["branch", "-M", "main"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to create main branch");

    // Create and switch to a new branch
    Command::new("git")
        .args(["checkout", "-b", "feature-branch"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to create feature branch");

    // Modify existing file
    fs::write(tree.root.join("file1.txt"), "modified content").expect("Failed to modify file1.txt");

    // Add new file
    fs::create_dir_all(tree.root.join("dir2")).expect("Failed to create dir2");
    fs::write(tree.root.join("dir2/file3.txt"), "new file content")
        .expect("Failed to create file3.txt");

    // Delete a file
    fs::remove_file(tree.root.join("dir1/file2.txt")).expect("Failed to delete file2.txt");

    // Stage and commit changes
    Command::new("git")
        .args(["add", "--all"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to stage changes");
    Command::new("git")
        .args(["commit", "-m", "Update files"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to commit changes");

    // Switch back to main branch to ensure it's properly tracked
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to checkout main branch");

    // Switch back to feature branch for testing
    Command::new("git")
        .args(["checkout", "feature-branch"])
        .current_dir(&tree.root)
        .output()
        .expect("Failed to checkout feature branch");

    (tree, output_tree)
}

#[test]
fn test_export_branch_changes() {
    let (repo_tree, output_tree) = setup_test_repo();
    let processor =
        git_changes::new(repo_tree.root.to_str().unwrap()).expect("Failed to create processor");

    let changes = processor
        .export_branch_changes("feature-branch", "main", &output_tree.root)
        .expect("Failed to export branch changes");

    assert_eq!(changes.len(), 3, "Should have 3 changed files");

    assert!(
        output_tree.root.join("file1.txt").exists(),
        "Modified file should exist"
    );
    assert_eq!(
        fs::read_to_string(output_tree.root.join("file1.txt")).expect("Failed to read file1.txt"),
        "modified content"
    );
    assert!(
        output_tree.root.join("dir2/file3.txt").exists(),
        "New file should exist"
    );
    assert_eq!(
        fs::read_to_string(output_tree.root.join("dir2/file3.txt"))
            .expect("Failed to read file3.txt"),
        "new file content"
    );
    assert!(
        !output_tree.root.join("dir1/file2.txt").exists(),
        "Deleted file should not exist in output"
    );

    let diff_file_path = output_tree.root.join("file1.txt.diff");
    assert!(
        diff_file_path.exists(),
        "Diff file for file1.txt should exist"
    );
    let diff_content = fs::read_to_string(&diff_file_path).expect("Failed to read diff file");
    assert!(diff_content.contains("-original content"));
    assert!(diff_content.contains("+modified content"));

    assert!(!output_tree.root.join("dir2/file3.txt.diff").exists());
    assert!(!output_tree.root.join("dir1/file2.txt.diff").exists());

    let file1_change = changes
        .get("file1.txt")
        .expect("file1.txt should be in changes");
    assert!(matches!(file1_change.status, FileStatus::Modified));
    let file2_change = changes
        .get("dir1/file2.txt")
        .expect("dir1/file2.txt should be in changes");
    assert!(matches!(file2_change.status, FileStatus::Deleted));
    let file3_change = changes
        .get("dir2/file3.txt")
        .expect("dir2/file3.txt should be in changes");
    assert!(matches!(file3_change.status, FileStatus::Added));
}

#[test]
fn test_list_only_mode_branch() {
    let (repo_tree, _output_tree) = setup_test_repo();
    let processor =
        git_changes::new(repo_tree.root.to_str().unwrap()).expect("Failed to create processor");

    let changes = processor
        .list_branch_changes("feature-branch", "main")
        .expect("Failed to list branch changes");

    assert_eq!(changes.len(), 3, "Should have 3 changed files");
    assert!(
        !Path::new(repo_tree.root.to_str().unwrap())
            .join("file1.txt.diff")
            .exists(),
        "Diff file should not be created in list_only mode"
    );
    assert!(!_output_tree.root.join("file1.txt").exists());
    assert!(!_output_tree.root.join("dir2/file3.txt").exists());

    let file1_change = changes
        .get("file1.txt")
        .expect("file1.txt should be in changes");
    assert!(matches!(file1_change.status, FileStatus::Modified));
    let file2_change = changes
        .get("dir1/file2.txt")
        .expect("dir1/file2.txt should be in changes");
    assert!(matches!(file2_change.status, FileStatus::Deleted));
    let file3_change = changes
        .get("dir2/file3.txt")
        .expect("dir2/file3.txt should be in changes");
    assert!(matches!(file3_change.status, FileStatus::Added));
}

#[test]
fn test_export_commit_changes() {
    let (repo_tree, output_tree) = setup_test_repo();
    let commit_hash = Command::new("git")
        .args(["rev-parse", "feature-branch"])
        .current_dir(&repo_tree.root)
        .output()
        .expect("Failed to get commit hash")
        .stdout;
    let commit_hash_str = String::from_utf8(commit_hash)
        .expect("Failed to parse commit hash")
        .trim()
        .to_string();

    let processor =
        git_changes::new(repo_tree.root.to_str().unwrap()).expect("Failed to create processor");

    let changes = processor
        .export_commit_changes(&commit_hash_str, &output_tree.root)
        .expect("Failed to export commit changes");

    assert!(output_tree.root.join("file1.txt").exists());
    assert_eq!(
        fs::read_to_string(output_tree.root.join("file1.txt")).expect("Failed to read file1.txt"),
        "modified content"
    );
    assert!(output_tree.root.join("dir2/file3.txt").exists());
    assert_eq!(
        fs::read_to_string(output_tree.root.join("dir2/file3.txt"))
            .expect("Failed to read file3.txt"),
        "new file content"
    );
    assert!(!output_tree.root.join("dir1/file2.txt").exists());

    let diff_file_path = output_tree.root.join("file1.txt.diff");
    assert!(
        diff_file_path.exists(),
        "Diff file for file1.txt should exist for commit changes"
    );
    let diff_content = fs::read_to_string(&diff_file_path).expect("Failed to read diff file");
    assert!(diff_content.contains("-original content"));
    assert!(diff_content.contains("+modified content"));

    assert!(!output_tree.root.join("dir2/file3.txt.diff").exists());
    assert!(!output_tree.root.join("dir1/file2.txt.diff").exists());

    let file1_change = changes
        .get("file1.txt")
        .expect("file1.txt should be in changes");
    assert!(matches!(file1_change.status, FileStatus::Modified));
    let file2_change = changes
        .get("dir1/file2.txt")
        .expect("dir1/file2.txt should be in changes");
    assert!(matches!(file2_change.status, FileStatus::Deleted));
    let file3_change = changes
        .get("dir2/file3.txt")
        .expect("dir2/file3.txt should be in changes");
    assert!(matches!(file3_change.status, FileStatus::Added));
}
