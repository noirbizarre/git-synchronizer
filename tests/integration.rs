//! Integration tests exercising the compiled binary end-to-end via `assert_cmd`.
//!
//! These cover the `main.rs` entry point, `handle_config_command`, and
//! `handle_clean` — code paths that are unreachable from unit tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Command as StdCommand;
use tempfile::TempDir;

// ── Helpers ──────────────────────────────────────────────────────────

/// Initialize a minimal git repo with a single commit on `main`.
fn init_repo() -> TempDir {
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

/// Seed the `[sync]` config section so the clean workflow
/// doesn't trigger the interactive setup wizard.
fn configure(dir: &TempDir) {
    let p = dir.path();
    StdCommand::new("git")
        .args(["config", "--add", "sync.protected", "main"])
        .current_dir(p)
        .output()
        .unwrap();
}

/// Add a merged branch (`feature/done`) and an unmerged branch (`feature/wip`).
fn add_branches(dir: &TempDir) {
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

/// Return the list of local branch names in the repo.
fn git_branches(dir: &TempDir) -> Vec<String> {
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

// ── CLI basics ───────────────────────────────────────────────────────

#[test]
fn help_flag_shows_usage() {
    Command::cargo_bin("git-sync")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Easily synchronize your local branches",
        ));
}

#[test]
fn version_flag_shows_version() {
    Command::cargo_bin("git-sync")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("git-sync"));
}

// ── Config subcommands ───────────────────────────────────────────────

#[test]
fn config_list_no_config() {
    let dir = init_repo();
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("No configuration found"));
}

#[test]
fn config_list_shows_values() {
    let dir = init_repo();
    configure(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("protected:"))
        .stderr(predicate::str::contains("main"))
        .stderr(predicate::str::contains("(all)"));
}

#[test]
fn config_list_with_configured_remotes() {
    let dir = init_repo();
    let p = dir.path();

    // Set up a config with specific remotes
    StdCommand::new("git")
        .args(["config", "--add", "sync.protected", "main"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "--add", "sync.remote", "origin"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "--add", "sync.remote", "upstream"])
        .current_dir(p)
        .output()
        .unwrap();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains("origin, upstream"));
}

#[test]
fn config_set_value() {
    let dir = init_repo();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "set", "remote", "upstream"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Set remote = upstream"));

    // Verify with git config
    let output = StdCommand::new("git")
        .args(["config", "--get", "sync.remote"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "upstream");
}

#[test]
fn config_add_and_remove_protected() {
    let dir = init_repo();
    let p = dir.path();

    // Add a protected pattern
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-protected", "release/*"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Added protected pattern: release/*",
        ));

    // Add another
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-protected", "main"])
        .current_dir(p)
        .assert()
        .success();

    // Verify both exist
    let output = StdCommand::new("git")
        .args(["config", "--get-all", "sync.protected"])
        .current_dir(p)
        .output()
        .unwrap();
    let values = String::from_utf8_lossy(&output.stdout);
    assert!(values.contains("release/*"));
    assert!(values.contains("main"));

    // Remove one
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "remove-protected", "release/*"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Removed protected pattern: release/*",
        ));

    // Verify only main remains
    let output = StdCommand::new("git")
        .args(["config", "--get-all", "sync.protected"])
        .current_dir(p)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "main");
}

#[test]
fn config_add_and_remove_remote() {
    let dir = init_repo();
    let p = dir.path();

    // Add remotes
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-remote", "origin"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains("Added remote: origin"));

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-remote", "upstream"])
        .current_dir(p)
        .assert()
        .success();

    // Remove one
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "remove-remote", "upstream"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains("Removed remote: upstream"));

    // Verify only origin remains
    let output = StdCommand::new("git")
        .args(["config", "--get-all", "sync.remote"])
        .current_dir(p)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "origin");
}

// ── Clean workflow ───────────────────────────────────────────────────

#[test]
fn clean_dry_run_preserves_branches() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "-n", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("dry-run"))
        .stderr(predicate::str::contains("feature/done"));

    // Branches must still exist
    let branches = git_branches(&dir);
    assert!(branches.contains(&"feature/done".to_string()));
    assert!(branches.contains(&"feature/wip".to_string()));
}

#[test]
fn clean_deletes_merged_branch() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success();

    let branches = git_branches(&dir);
    assert!(!branches.contains(&"feature/done".to_string()));
    assert!(branches.contains(&"feature/wip".to_string()));
    assert!(branches.contains(&"main".to_string()));
}

#[test]
fn clean_no_merged_branches() {
    let dir = init_repo();
    configure(&dir);
    // No extra branches — nothing to delete

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("No merged local branches"));
}

#[test]
fn clean_remote_only_skips_local_deletion() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch", "--remote-only"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Local merged branch must NOT be deleted
    let branches = git_branches(&dir);
    assert!(branches.contains(&"feature/done".to_string()));
}

#[test]
fn clean_local_only_deletes_local() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch", "--local-only"])
        .current_dir(dir.path())
        .assert()
        .success();

    let branches = git_branches(&dir);
    assert!(!branches.contains(&"feature/done".to_string()));
}

// ── Per-branch protection ───────────────────────────────────────────

#[test]
fn config_protect_and_unprotect() {
    let dir = init_repo();
    let p = dir.path();

    // Protect a branch
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "protect", "develop"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Branch 'develop' marked as protected",
        ));

    // Verify with git config
    let output = StdCommand::new("git")
        .args(["config", "--get", "branch.develop.sync-protected"])
        .current_dir(p)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "true");

    // Unprotect
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "unprotect", "develop"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Branch 'develop' is no longer protected",
        ));

    // Verify key is removed
    let output = StdCommand::new("git")
        .args(["config", "--get", "branch.develop.sync-protected"])
        .current_dir(p)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "key should be unset after unprotect"
    );
}

#[test]
fn config_list_shows_branch_protected() {
    let dir = init_repo();
    let p = dir.path();
    configure(&dir);

    // Mark a branch as per-branch protected
    StdCommand::new("git")
        .args(["config", "branch.staging.sync-protected", "true"])
        .current_dir(p)
        .output()
        .unwrap();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains("branch protected:"))
        .stderr(predicate::str::contains("staging"));
}

#[test]
fn clean_respects_branch_protected() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    // Mark the merged branch as per-branch protected
    let p = dir.path();
    StdCommand::new("git")
        .args(["config", "branch.feature/done.sync-protected", "true"])
        .current_dir(p)
        .output()
        .unwrap();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(p)
        .assert()
        .success();

    // feature/done should NOT be deleted because it is per-branch protected
    let branches = git_branches(&dir);
    assert!(
        branches.contains(&"feature/done".to_string()),
        "per-branch protected branch should not be deleted"
    );
    // main should still exist
    assert!(branches.contains(&"main".to_string()));
}

// ── Worktree config support ─────────────────────────────────────────

/// Initialize a repo with `extensions.worktreeConfig = true` and a linked
/// worktree. Returns (tempdir, main_path, worktree_path).
fn init_repo_with_worktree_config() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
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

#[test]
fn config_set_from_linked_worktree_visible_in_main() {
    let (_dir, main_path, wt_path) = init_repo_with_worktree_config();

    // Run config commands from the linked worktree
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-protected", "main"])
        .current_dir(&wt_path)
        .assert()
        .success();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-protected", "release/*"])
        .current_dir(&wt_path)
        .assert()
        .success();

    // Config should be visible from the main worktree
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(&main_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("main"))
        .stderr(predicate::str::contains("release/*"));
}

#[test]
fn config_protect_from_linked_worktree_visible_in_main() {
    let (_dir, main_path, wt_path) = init_repo_with_worktree_config();

    // Seed minimal config so list doesn't show "no config"
    StdCommand::new("git")
        .args(["config", "--local", "--add", "sync.protected", "main"])
        .current_dir(&main_path)
        .output()
        .unwrap();

    // Protect a branch from the linked worktree
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "protect", "develop"])
        .current_dir(&wt_path)
        .assert()
        .success();

    // Branch protection should be visible from the main worktree
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(&main_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("develop"));
}

#[test]
fn clean_from_linked_worktree_with_worktree_config() {
    let (_dir, main_path, wt_path) = init_repo_with_worktree_config();

    // Configure from main worktree
    StdCommand::new("git")
        .args(["config", "--local", "--add", "sync.protected", "main"])
        .current_dir(&main_path)
        .output()
        .unwrap();

    // Create and merge a branch from the main worktree
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/done"])
        .current_dir(&main_path)
        .output()
        .unwrap();
    std::fs::write(main_path.join("done.txt"), "done").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&main_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "done"])
        .current_dir(&main_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(&main_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["merge", "feature/done"])
        .current_dir(&main_path)
        .output()
        .unwrap();

    // Run clean from the linked worktree — must succeed and see config
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch", "--no-worktrees"])
        .current_dir(&wt_path)
        .assert()
        .success();

    // The merged branch should have been deleted
    let output = StdCommand::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(&main_path)
        .output()
        .unwrap();
    let branches: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();
    assert!(
        !branches.contains(&"feature/done".to_string()),
        "merged branch should be deleted when running from linked worktree"
    );
    assert!(branches.contains(&"main".to_string()));
}
