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

/// Create a clone whose `feature/gone` branch tracks a deleted remote branch.
///
/// Layout of the returned working clone:
/// - `main` — protected target, tracks `origin/main`
/// - `feature/gone` — one unmerged commit, upstream deleted then pruned
/// - `feature/alive` — one unmerged commit, upstream still present
/// - `feature/done` — merged into `main`, no upstream
///
/// Returns the tempdir holding both repos and a [`Git`] rooted in the clone.
pub fn init_repo_with_gone_upstream() -> Result<(TempDir, Git)> {
    let dir = tempfile::tempdir()?;
    let origin = dir.path().join("origin.git");
    let work = dir.path().join("work");

    let git_in = |cwd: &std::path::Path, args: &[&str]| -> Result<()> {
        StdCommand::new("git")
            .args(args)
            .current_dir(cwd)
            .output()?;
        Ok(())
    };

    // Seed repository, then turn it into the remote.
    let (seed, _) = init_repo()?;
    git_in(
        seed.path(),
        &["clone", "--bare", ".", origin.to_str().unwrap()],
    )?;
    git_in(
        dir.path(),
        &["clone", origin.to_str().unwrap(), work.to_str().unwrap()],
    )?;
    git_in(&work, &["config", "user.email", "test@test.com"])?;
    git_in(&work, &["config", "user.name", "Test"])?;

    for branch in ["feature/gone", "feature/alive"] {
        git_in(&work, &["checkout", "-b", branch, "main"])?;
        let file = format!("{}.txt", branch.replace('/', "-"));
        std::fs::write(work.join(file), branch)?;
        git_in(&work, &["add", "."])?;
        git_in(&work, &["commit", "-m", branch])?;
        git_in(&work, &["push", "-u", "origin", branch])?;
    }

    // A branch merged the classic way, with no upstream at all.
    git_in(&work, &["checkout", "-b", "feature/done", "main"])?;
    std::fs::write(work.join("done.txt"), "done")?;
    git_in(&work, &["add", "."])?;
    git_in(&work, &["commit", "-m", "feature done"])?;
    git_in(&work, &["checkout", "main"])?;
    git_in(&work, &["merge", "--no-ff", "-m", "merge", "feature/done"])?;

    // Drop the remote branch and prune, leaving `feature/gone` orphaned.
    git_in(&origin, &["branch", "-D", "feature/gone"])?;
    git_in(&work, &["fetch", "--prune", "origin"])?;

    let git = Git::with_workdir(false, &work);
    // Keep `seed` alive for the lifetime of the clone's object store copy.
    drop(seed);
    Ok((dir, git))
}

/// Create a repo with a merged branch, a linked worktree for it, and lock the worktree.
pub fn init_repo_with_locked_worktree() -> Result<(TempDir, Git, String)> {
    let (dir, git) = init_repo()?;
    let path = dir.path();

    // Create and merge a feature branch
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/locked-wt"])
        .current_dir(path)
        .output()?;
    std::fs::write(path.join("locked.txt"), "locked feature")?;
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "locked feature"])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(path)
        .output()?;
    StdCommand::new("git")
        .args(["merge", "feature/locked-wt"])
        .current_dir(path)
        .output()?;

    // Create a worktree for the merged branch
    let wt_path = dir.path().join("worktree-locked");
    StdCommand::new("git")
        .args([
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "feature/locked-wt",
        ])
        .current_dir(path)
        .output()?;

    // Lock the worktree
    StdCommand::new("git")
        .args(["worktree", "lock", wt_path.to_str().unwrap()])
        .current_dir(path)
        .output()?;

    Ok((dir, git, wt_path.to_string_lossy().to_string()))
}
