use clap::{ArgGroup, Parser};
use git_changes::{self, FileStatus};
use std::path::PathBuf;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(group(
    ArgGroup::new("target")
        .required(true)
        .args(["branch", "commit"]),
))]
struct Cli {
    /// Git repository (HTTPS/SSH URL or local path)
    #[arg(short, long)]
    repo: String,

    /// Branch to analyze (if not provided, will try to detect from CI environment)
    #[arg(short, long, group = "target")]
    branch: Option<String>,

    /// Target branch to compare against (defaults to origin/HEAD or origin/main if not found)
    #[arg(short = 't', long)]
    target_branch: Option<String>,

    /// Commit to analyze
    #[arg(short, long, group = "target")]
    commit: Option<String>,

    /// Output directory for changes (if not provided, only lists changes)
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// Log level
    #[arg(global = true, short, long, value_enum, default_value = "error")]
    log: LevelFilter,
}

fn print_changes_summary(changes: &std::collections::HashMap<String, git_changes::FileChange>) {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();

    for (path, change) in changes {
        match change.status {
            FileStatus::Added => added.push(path),
            FileStatus::Modified => modified.push(path),
            FileStatus::Deleted => deleted.push(path),
        }
    }

    added.sort();
    modified.sort();
    deleted.sort();

    println!("\n📊 Changes Summary:");
    println!("==================");
    println!("Total files: {}", changes.len());
    println!("  Added:    {}", added.len());
    println!("  Modified: {}", modified.len());
    println!("  Deleted:  {}", deleted.len());

    if !added.is_empty() {
        println!("\n✨ Added Files:");
        for path in added {
            println!("  + {path}");
        }
    }

    if !modified.is_empty() {
        println!("\n🔄 Modified Files:");
        for path in modified {
            println!("  ~ {path}");
        }
    }

    if !deleted.is_empty() {
        println!("\n❌ Deleted Files:");
        for path in deleted {
            println!("  - {path}");
        }
    }
}

#[tokio::main]
async fn main() -> git_changes::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let env_filter = EnvFilter::from_default_env().add_directive(cli.log.into());

    fmt()
        .with_env_filter(env_filter)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .pretty()
        .init();

    let processor = git_changes::new(&cli.repo)?;

    let changes = if let Some(output_dir) = &cli.output_dir {
        if let Some(branch) = cli.branch {
            if let Some(target_branch) = &cli.target_branch {
                processor.export_branch_changes(&branch, target_branch, output_dir)?
            } else {
                processor.export_changes_from_default_branch(&branch, output_dir)?
            }
        } else if let Some(commit) = cli.commit {
            processor.export_commit_changes(&commit, output_dir)?
        } else {
            unreachable!("ArgGroup ensures exactly one target is provided")
        }
    } else if let Some(branch) = cli.branch {
        if let Some(target_branch) = &cli.target_branch {
            processor.list_branch_changes(&branch, target_branch)?
        } else {
            processor.list_changes_from_default_branch(&branch)?
        }
    } else if let Some(commit) = cli.commit {
        processor.list_commit_changes(&commit)?
    } else {
        unreachable!("ArgGroup ensures exactly one target is provided")
    };

    print_changes_summary(&changes);

    Ok(())
}
