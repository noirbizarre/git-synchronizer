//! Shared fixtures for the integration tests.
//!
//! Integration tests are compiled as a separate crate and therefore cannot
//! reach `crate::test_helpers`; this module mirrors it for the binary-level
//! tests. Helpers panic on failure rather than returning `Result`, so a broken
//! fixture surfaces as a test failure at the exact setup step.

use std::process::Command as StdCommand;
use tempfile::TempDir;

/// A pid far beyond any real OS's pid_max (Linux tops out at 4194304 even at
/// its highest configurable ceiling; macOS/BSD/Windows are far lower), so it
/// is guaranteed dead without spawning and reaping a process. Mirrors
/// `crate::test_helpers::DEAD_PID`.
pub const DEAD_PID: u32 = 4_000_000_000;

/// Initialize a minimal git repo with a single commit on `main`.
pub fn init_repo() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();

    StdCommand::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(p)
        .output()
        .unwrap();

    std::fs::write(p.join("README.md"), "# test").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(p)
        .output()
        .unwrap();

    dir
}

/// Seed the `[wipe]` config section so the clean workflow
/// doesn't trigger the interactive setup wizard.
pub fn configure(dir: &TempDir) {
    let p = dir.path();
    StdCommand::new("git")
        .args(["config", "--add", "wipe.protected", "main"])
        .current_dir(p)
        .output()
        .unwrap();
}

/// Add a merged branch (`feature/done`) and an unmerged branch (`feature/wip`).
pub fn add_branches(dir: &TempDir) {
    let p = dir.path();

    // Create and merge feature/done
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/done"])
        .current_dir(p)
        .output()
        .unwrap();
    std::fs::write(p.join("done.txt"), "done").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "done"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["merge", "feature/done"])
        .current_dir(p)
        .output()
        .unwrap();

    // Create unmerged feature/wip
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/wip"])
        .current_dir(p)
        .output()
        .unwrap();
    std::fs::write(p.join("wip.txt"), "wip").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "wip"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(p)
        .output()
        .unwrap();
}

/// Add a linked worktree on a branch already merged into `main`, so the
/// worktree is a removal candidate. Returns its path.
///
/// The worktree is necessarily brand new, which is exactly what the
/// `--min-age` guard is meant to protect.
pub fn add_merged_worktree(dir: &TempDir, branch: &str, name: &str) -> std::path::PathBuf {
    let p = dir.path();

    for args in [
        vec!["checkout", "-b", branch],
        vec!["commit", "--allow-empty", "-m", "worktree work"],
        vec!["checkout", "main"],
        vec!["merge", branch, "--no-edit"],
    ] {
        StdCommand::new("git")
            .args(&args)
            .current_dir(p)
            .output()
            .unwrap();
    }

    let wt_path = p.join(name);
    StdCommand::new("git")
        .args(["worktree", "add", wt_path.to_str().unwrap(), branch])
        .current_dir(p)
        .output()
        .unwrap();
    assert!(
        wt_path.exists(),
        "fixture worktree should have been created"
    );

    wt_path
}

/// Return the list of local branch names in the repo.
pub fn git_branches(dir: &TempDir) -> Vec<String> {
    let output = StdCommand::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Initialize a repo with `extensions.worktreeConfig = true` and a linked
/// worktree. Returns (tempdir, main_path, worktree_path).
pub fn init_repo_with_worktree_config() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let main_path = dir.path().join("main-repo");
    std::fs::create_dir_all(&main_path).unwrap();

    StdCommand::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(&main_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&main_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&main_path)
        .output()
        .unwrap();

    std::fs::write(main_path.join("README.md"), "# test").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&main_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&main_path)
        .output()
        .unwrap();

    // Enable extensions.worktreeConfig
    StdCommand::new("git")
        .args(["config", "extensions.worktreeConfig", "true"])
        .current_dir(&main_path)
        .output()
        .unwrap();

    // Create a branch and a linked worktree
    StdCommand::new("git")
        .args(["branch", "feature/wt"])
        .current_dir(&main_path)
        .output()
        .unwrap();
    let wt_path = dir.path().join("linked-wt");
    StdCommand::new("git")
        .args(["worktree", "add", wt_path.to_str().unwrap(), "feature/wt"])
        .current_dir(&main_path)
        .output()
        .unwrap();

    (dir, main_path, wt_path)
}
