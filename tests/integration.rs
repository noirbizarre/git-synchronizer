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

// ── Locked worktree handling ────────────────────────────────────────

#[test]
fn clean_skips_locked_worktree() {
    let dir = init_repo();
    let p = dir.path();
    configure(&dir);

    // Create and merge a branch
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/locked"])
        .current_dir(p)
        .output()
        .unwrap();
    std::fs::write(p.join("locked.txt"), "locked feature").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "locked feature"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["merge", "feature/locked"])
        .current_dir(p)
        .output()
        .unwrap();

    // Create a worktree for the merged branch and lock it
    let wt_path = p.join("wt-locked");
    StdCommand::new("git")
        .args([
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "feature/locked",
        ])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["worktree", "lock", wt_path.to_str().unwrap()])
        .current_dir(p)
        .output()
        .unwrap();

    // Run clean — should skip the locked worktree
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains("Skipping locked worktree"));

    // The locked worktree directory should still exist
    assert!(wt_path.exists(), "locked worktree should not be removed");

    // The branch cannot be deleted because it's still checked out in
    // the locked worktree — git refuses to delete it. This is expected:
    // the worktree removal was skipped, so branch deletion also fails
    // gracefully (logged as a warning).
    let branches = git_branches(&dir);
    assert!(
        branches.contains(&"feature/locked".to_string()),
        "branch should survive because its locked worktree prevents deletion"
    );
}

#[test]
fn clean_skips_locked_worktree_with_reason() {
    let dir = init_repo();
    let p = dir.path();
    configure(&dir);

    // Create and merge a branch
    StdCommand::new("git")
        .args(["checkout", "-b", "feature/locked-reason"])
        .current_dir(p)
        .output()
        .unwrap();
    std::fs::write(p.join("reason.txt"), "reason feature").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "reason feature"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["merge", "feature/locked-reason"])
        .current_dir(p)
        .output()
        .unwrap();

    // Create a worktree and lock it with a reason
    let wt_path = p.join("wt-locked-reason");
    StdCommand::new("git")
        .args([
            "worktree",
            "add",
            wt_path.to_str().unwrap(),
            "feature/locked-reason",
        ])
        .current_dir(p)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args([
            "worktree",
            "lock",
            wt_path.to_str().unwrap(),
            "--reason",
            "work in progress",
        ])
        .current_dir(p)
        .output()
        .unwrap();

    // Run clean — should show the lock reason
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(p)
        .assert()
        .success()
        .stderr(predicate::str::contains("Skipping locked worktree"))
        .stderr(predicate::str::contains("work in progress"));

    // Locked worktree must still exist
    assert!(wt_path.exists(), "locked worktree should not be removed");
}

// ── Pull / fast-forward ─────────────────────────────────────────────

/// Create a local bare "remote", clone it, push an initial commit, and
/// configure sync.  Returns (tempdir, work_path, bare_path).
fn init_repo_with_remote() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();

    // Bare remote
    let bare_path = dir.path().join("remote.git");
    StdCommand::new("git")
        .args([
            "init",
            "--bare",
            "--initial-branch=main",
            bare_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Clone → working repo
    let work_path = dir.path().join("work");
    StdCommand::new("git")
        .args([
            "clone",
            bare_path.to_str().unwrap(),
            work_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&work_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Initial commit + push
    std::fs::write(work_path.join("README.md"), "# test").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&work_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&work_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push", "-u", "origin", "main"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Configure sync
    StdCommand::new("git")
        .args(["config", "--add", "sync.protected", "main"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    (dir, work_path, bare_path)
}

/// Push a new commit to the bare remote from a temporary second clone.
fn advance_remote_branch(bare_path: &std::path::Path, parent_dir: &std::path::Path) {
    let pusher = parent_dir.join("pusher");
    StdCommand::new("git")
        .args([
            "clone",
            bare_path.to_str().unwrap(),
            pusher.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    std::fs::write(pusher.join("remote-new.txt"), "remote content").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "advance remote"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push"])
        .current_dir(&pusher)
        .output()
        .unwrap();
}

/// Return the SHA of a ref in a repo.
fn git_rev_parse(dir: &std::path::Path, refname: &str) -> String {
    let output = StdCommand::new("git")
        .args(["rev-parse", refname])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn no_pull_flag_accepted() {
    let dir = init_repo();
    configure(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch", "--no-pull"])
        .current_dir(dir.path())
        .assert()
        .success();
}

#[test]
fn pull_updates_current_branch() {
    let (dir, work_path, bare_path) = init_repo_with_remote();

    // Advance remote
    advance_remote_branch(&bare_path, dir.path());

    let before = git_rev_parse(&work_path, "HEAD");

    // Run git-sync with pull enabled (default), fetch enabled
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y"])
        .current_dir(&work_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Pulling"));

    let after = git_rev_parse(&work_path, "HEAD");
    assert_ne!(before, after, "main should have been fast-forwarded");
    assert!(
        work_path.join("remote-new.txt").exists(),
        "new file from remote should exist after pull"
    );
}

#[test]
fn no_pull_skips_fast_forward() {
    let (dir, work_path, bare_path) = init_repo_with_remote();

    // Advance remote
    advance_remote_branch(&bare_path, dir.path());

    let before = git_rev_parse(&work_path, "HEAD");

    // Run git-sync with --no-pull
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-pull"])
        .current_dir(&work_path)
        .assert()
        .success();

    let after = git_rev_parse(&work_path, "HEAD");
    assert_eq!(
        before, after,
        "main should NOT have been updated with --no-pull"
    );
}

#[test]
fn pull_updates_branch_in_worktree() {
    let (dir, work_path, bare_path) = init_repo_with_remote();

    // Create a second protected branch, push it, check out in a worktree
    StdCommand::new("git")
        .args(["checkout", "-b", "develop"])
        .current_dir(&work_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push", "-u", "origin", "develop"])
        .current_dir(&work_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Add develop as protected
    StdCommand::new("git")
        .args(["config", "--add", "sync.protected", "develop"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Create a worktree for develop
    let wt_path = dir.path().join("wt-develop");
    StdCommand::new("git")
        .args(["worktree", "add", wt_path.to_str().unwrap(), "develop"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Advance develop on the remote
    let pusher = dir.path().join("pusher-dev");
    StdCommand::new("git")
        .args([
            "clone",
            "-b",
            "develop",
            bare_path.to_str().unwrap(),
            pusher.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    std::fs::write(pusher.join("dev-new.txt"), "dev content").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "advance develop"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push"])
        .current_dir(&pusher)
        .output()
        .unwrap();

    let before = git_rev_parse(&work_path, "develop");

    // Run git-sync — should pull develop via the worktree
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y"])
        .current_dir(&work_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Pulling"));

    let after = git_rev_parse(&work_path, "develop");
    assert_ne!(
        before, after,
        "develop should have been fast-forwarded via worktree"
    );
    assert!(
        wt_path.join("dev-new.txt").exists(),
        "new file should be in the worktree after pull"
    );
}

#[test]
fn pull_updates_non_checked_out_branch_via_fetch() {
    let (dir, work_path, bare_path) = init_repo_with_remote();

    // Create a second protected branch, push it, but do NOT check it out in a worktree
    StdCommand::new("git")
        .args(["checkout", "-b", "develop"])
        .current_dir(&work_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push", "-u", "origin", "develop"])
        .current_dir(&work_path)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Add develop as protected
    StdCommand::new("git")
        .args(["config", "--add", "sync.protected", "develop"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Advance develop on the remote
    let pusher = dir.path().join("pusher-dev2");
    StdCommand::new("git")
        .args([
            "clone",
            "-b",
            "develop",
            bare_path.to_str().unwrap(),
            pusher.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    std::fs::write(pusher.join("dev-new2.txt"), "dev content 2").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "advance develop 2"])
        .current_dir(&pusher)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push"])
        .current_dir(&pusher)
        .output()
        .unwrap();

    let before = git_rev_parse(&work_path, "develop");

    // Run git-sync — should update develop via fetch refspec
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y"])
        .current_dir(&work_path)
        .assert()
        .success();

    let after = git_rev_parse(&work_path, "develop");
    assert_ne!(
        before, after,
        "develop should have been fast-forwarded via fetch"
    );
}

#[test]
fn pull_dry_run_does_not_update() {
    let (dir, work_path, bare_path) = init_repo_with_remote();

    // Advance remote
    advance_remote_branch(&bare_path, dir.path());

    let before = git_rev_parse(&work_path, "HEAD");

    // Run git-sync with --dry-run
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "-n"])
        .current_dir(&work_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("dry-run"));

    let after = git_rev_parse(&work_path, "HEAD");
    assert_eq!(before, after, "HEAD should NOT change in dry-run mode");
}
