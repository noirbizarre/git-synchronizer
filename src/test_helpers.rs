//! Shared test helpers for creating temporary git repositories.
//!
//! These helpers are only compiled in test builds.

use std::process::Command as StdCommand;

use anyhow::Result;
use tempfile::TempDir;

use crate::git::Git;

/// Initialize a minimal git repo with a single commit on `main`.
pub fn init_repo() -> Result<(TempDir, Git)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path();

    StdCommand::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(path)
        .output()?;

    std::fs::write(path.join("README.md"), "# test")?;
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(path)
        .output()?;

    let git = Git::with_workdir(false, path);
    Ok((dir, git))
}

/// Create a repo with a merged branch `feature/done` and an unmerged branch `feature/wip`.
pub fn init_repo_with_branches() -> Result<(TempDir, Git)> {
    let (dir, git) = init_repo()?;
    let path = dir.path();

    // Create and merge a feature branch
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/done"])
        .current_dir(path)
        .output()?;
    std::fs::write(path.join("done.txt"), "done")?;
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "feature done"])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["merge", "feature/done"])
        .current_dir(path)
        .output()?;

    // Create an unmerged branch
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/wip"])
        .current_dir(path)
        .output()?;
    std::fs::write(path.join("wip.txt"), "wip")?;
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "work in progress"])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(path)
        .output()?;

    Ok((dir, git))
}

/// Create a repo with a branch and a linked worktree for it.
pub fn init_repo_with_worktree() -> Result<(TempDir, Git, String)> {
    let (dir, git) = init_repo()?;
    let path = dir.path();

    StdCommand::new("git")
        .args(["branch", "feature/wt"])
        .current_dir(path)
        .output()?;

    let wt_path = dir.path().join("worktree-feature");
    StdCommand::new("git")
        .args(["worktree", "add", wt_path.to_str().unwrap(), "feature/wt"])
        .current_dir(path)
        .output()?;

    Ok((dir, git, wt_path.to_string_lossy().to_string()))
}
