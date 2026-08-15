//! Shared test helpers for creating temporary git repositories.
//!
//! These helpers are only compiled in test builds.

use std::path::Path;
use std::process::Command as StdCommand;

use anyhow::Result;
use tempfile::TempDir;

use crate::git::Git;

/// Run a git command in `cwd`, ignoring its output.
///
/// Fixture setup only: the exit status is deliberately not checked, because
/// several fixtures run commands that are allowed to be no-ops.
pub fn git_in(cwd: &Path, args: &[&str]) -> Result<()> {
    StdCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()?;
    Ok(())
}

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

/// Helper: create a repo with `extensions.worktreeConfig = true` and
/// a linked worktree, returning (tempdir, main_path, worktree_path).
pub fn init_repo_with_worktree_config() -> Result<(TempDir, std::path::PathBuf, std::path::PathBuf)>
{
    let dir = tempfile::tempdir()?;
    let main_path = dir.path().join("main-repo");
    std::fs::create_dir_all(&main_path)?;

    StdCommand::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(&main_path)
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&main_path)
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&main_path)
        .output()?;

    std::fs::write(main_path.join("README.md"), "# test")?;
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&main_path)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&main_path)
        .output()?;

    // Enable extensions.worktreeConfig
    StdCommand::new("git")
        .args(["config", "extensions.worktreeConfig", "true"])
        .current_dir(&main_path)
        .output()?;

    // Create a branch and a linked worktree
    StdCommand::new("git")
        .args(["branch", "feature/wt"])
        .current_dir(&main_path)
        .output()?;
    let wt_path = dir.path().join("linked-wt");
    StdCommand::new("git")
        .args(["worktree", "add", wt_path.to_str().unwrap(), "feature/wt"])
        .current_dir(&main_path)
        .output()?;

    Ok((dir, main_path, wt_path))
}

/// Helper: create a repo with a local bare "remote" and tracking set up.
/// Returns (tempdir, workdir_path, bare_remote_path).
pub fn init_repo_with_local_remote() -> Result<(TempDir, std::path::PathBuf, std::path::PathBuf)> {
    let dir = tempfile::tempdir()?;

    // Create a bare "remote" repo
    let bare_path = dir.path().join("remote.git");
    StdCommand::new("git")
        .args([
            "init",
            "--bare",
            "--initial-branch=main",
            bare_path.to_str().unwrap(),
        ])
        .output()?;

    // Clone it to get a working repo with tracking
    let work_path = dir.path().join("work");
    StdCommand::new("git")
        .args([
            "clone",
            bare_path.to_str().unwrap(),
            work_path.to_str().unwrap(),
        ])
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&work_path)
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&work_path)
        .output()?;

    // Create an initial commit and push
    std::fs::write(work_path.join("README.md"), "# test")?;
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&work_path)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&work_path)
        .output()?;
    StdCommand::new("git")
        .args(["push", "-u", "origin", "main"])
        .current_dir(&work_path)
        .output()?;

    Ok((dir, work_path, bare_path))
}

/// Advance the bare remote by pushing from a temporary second clone.
pub fn advance_remote(bare_path: &Path, dir: &Path) -> Result<()> {
    let pusher = dir.join("pusher");
    StdCommand::new("git")
        .args([
            "clone",
            bare_path.to_str().unwrap(),
            pusher.to_str().unwrap(),
        ])
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&pusher)
        .output()?;
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&pusher)
        .output()?;
    std::fs::write(pusher.join("new.txt"), "new content")?;
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&pusher)
        .output()?;
    StdCommand::new("git")
        .args(["commit", "-m", "remote advance"])
        .current_dir(&pusher)
        .output()?;
    StdCommand::new("git")
        .args(["push"])
        .current_dir(&pusher)
        .output()?;
    Ok(())
}
