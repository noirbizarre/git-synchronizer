//! Integration tests exercising the compiled binary end-to-end via `assert_cmd`.
//!
//! These cover the `main.rs` entry point, `handle_config_command`, and
//! `handle_clean` — code paths that are unreachable from unit tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::process::Command as StdCommand;
use tempfile::TempDir;

mod common;
use common::{
    add_branches, add_merged_worktree, configure, git_branches, init_repo,
    init_repo_with_worktree_config,
};

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
fn config_set_replaces_every_value_of_a_multi_valued_key() {
    let dir = init_repo();

    // Two values: a plain `git config sync.protected <v>` would fail here.
    for value in ["main", "develop"] {
        StdCommand::new("git")
            .args(["config", "--add", "sync.protected", value])
            .current_dir(dir.path())
            .output()
            .unwrap();
    }

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "set", "protected", "release"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Set protected = release"));

    let output = StdCommand::new("git")
        .args(["config", "--get-all", "sync.protected"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "release");
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
fn effort_flag_accepted() {
    let dir = init_repo();
    configure(&dir);

    for level in ["1", "2", "3"] {
        Command::cargo_bin("git-sync")
            .unwrap()
            .args(["-y", "--no-fetch", "--local-only", "--effort", level])
            .current_dir(dir.path())
            .assert()
            .success();
    }
}

#[test]
fn effort_flag_rejects_out_of_range_levels() {
    let dir = init_repo();
    configure(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch", "--effort", "4"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("4"));
}

#[test]
fn config_set_effort_roundtrips() {
    let dir = init_repo();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "set", "effort", "3"])
        .current_dir(dir.path())
        .assert()
        .success();

    let output = StdCommand::new("git")
        .args(["config", "--get", "sync.effort"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "3");

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("3 (thorough)"));
}

#[test]
fn min_age_flag_accepted() {
    let dir = init_repo();
    configure(&dir);

    for value in ["0", "30s", "15m", "2h", "7d", "1w"] {
        Command::cargo_bin("git-sync")
            .unwrap()
            .args(["-y", "--no-fetch", "--local-only", "--min-age", value])
            .current_dir(dir.path())
            .assert()
            .success();
    }
}

#[test]
fn min_age_flag_rejects_garbage() {
    let dir = init_repo();
    configure(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch", "--min-age", "soon"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid duration"));
}

#[test]
fn min_age_keeps_a_freshly_created_worktree() {
    let dir = init_repo();
    configure(&dir);
    let wt_path = add_merged_worktree(&dir, "feature/fresh", "wt-fresh");

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch", "--local-only", "--min-age", "1h"])
        .current_dir(dir.path())
        .assert()
        .success();

    assert!(
        wt_path.exists(),
        "a worktree created seconds ago must survive --min-age 1h"
    );
}

#[test]
fn config_set_minage_roundtrips() {
    let dir = init_repo();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "set", "minage", "2h"])
        .current_dir(dir.path())
        .assert()
        .success();

    let output = StdCommand::new("git")
        .args(["config", "--get", "sync.minage"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "2h");

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("min age"))
        .stderr(predicate::str::contains("2h"));
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

// ── Not a git repository ────────────────────────────────────────────

#[test]
fn exits_with_error_when_not_in_a_git_repo() {
    let dir = tempfile::tempdir().unwrap();

    Command::cargo_bin("git-sync")
        .unwrap()
        .current_dir(dir.path())
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn no_stack_trace_when_not_in_a_git_repo() {
    let dir = tempfile::tempdir().unwrap();

    Command::cargo_bin("git-sync")
        .unwrap()
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error:").not())
        .stderr(predicate::str::contains("stack backtrace").not());
}

// ── Deleted upstream detection ───────────────────────────────────────

/// Create a repo with a `feature/gone` branch whose remote branch was deleted,
/// plus a `feature/alive` branch still present on the remote. Neither branch is
/// merged into `main`, so only the deleted-upstream signal can find them.
fn init_repo_with_gone_upstream() -> (TempDir, std::path::PathBuf) {
    let (dir, work_path, bare_path) = init_repo_with_remote();

    for branch in ["feature/gone", "feature/alive"] {
        StdCommand::new("git")
            .args(["checkout", "-b", branch, "main"])
            .current_dir(&work_path)
            .output()
            .unwrap();
        std::fs::write(
            work_path.join(format!("{}.txt", branch.replace('/', "-"))),
            branch,
        )
        .unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&work_path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", branch])
            .current_dir(&work_path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["push", "-u", "origin", branch])
            .current_dir(&work_path)
            .output()
            .unwrap();
    }
    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    // Delete the remote branch, as a merge-and-delete PR workflow would.
    StdCommand::new("git")
        .args(["branch", "-D", "feature/gone"])
        .current_dir(&bare_path)
        .output()
        .unwrap();

    (dir, work_path)
}

/// Local branch names in an arbitrary directory.
fn branches_in(path: &std::path::Path) -> Vec<String> {
    let output = StdCommand::new("git")
        .args(["branch", "--format=%(refname:short)"])
        .current_dir(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

#[test]
fn gone_upstream_branch_is_reported_but_kept_by_default() {
    let (_dir, work_path) = init_repo_with_gone_upstream();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-pull", "--local-only"])
        .current_dir(&work_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("with a deleted upstream"));

    let branches = branches_in(&work_path);
    assert!(
        branches.contains(&"feature/gone".to_string()),
        "must not be deleted without --delete-gone: {branches:?}"
    );
}

#[test]
fn delete_gone_removes_branch_with_deleted_upstream() {
    let (_dir, work_path) = init_repo_with_gone_upstream();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-pull", "--local-only", "--delete-gone"])
        .current_dir(&work_path)
        .assert()
        .success();

    let branches = branches_in(&work_path);
    assert!(!branches.contains(&"feature/gone".to_string()));
    assert!(
        branches.contains(&"feature/alive".to_string()),
        "branches with a live upstream must survive: {branches:?}"
    );
}

#[test]
fn gone_upstream_detection_requires_a_fetch() {
    let (_dir, work_path) = init_repo_with_gone_upstream();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args([
            "-y",
            "--no-fetch",
            "--no-pull",
            "--local-only",
            "--delete-gone",
        ])
        .current_dir(&work_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("with a deleted upstream").not());

    assert!(branches_in(&work_path).contains(&"feature/gone".to_string()));
}

#[test]
fn gone_upstream_detection_runs_in_dry_run_without_fetch() {
    let (_dir, work_path) = init_repo_with_gone_upstream();

    // Refresh remote-tracking refs out of band, as a user would before a preview.
    StdCommand::new("git")
        .args(["fetch", "--prune", "origin"])
        .current_dir(&work_path)
        .output()
        .unwrap();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args([
            "-y",
            "--dry-run",
            "--no-fetch",
            "--no-pull",
            "--local-only",
            "--delete-gone",
        ])
        .current_dir(&work_path)
        .assert()
        .success()
        .stderr(predicate::str::contains("with a deleted upstream"))
        .stderr(predicate::str::contains(
            "(dry-run) Would delete local branch 'feature/gone'",
        ));

    assert!(branches_in(&work_path).contains(&"feature/gone".to_string()));
}

// ── Ignored branches ─────────────────────────────────────────────────

#[test]
fn ignored_branch_is_not_offered_for_deletion() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    StdCommand::new("git")
        .args(["config", "--add", "sync.ignore", "feature/*"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["--dry-run", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("feature/done").not());

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success();

    let branches = git_branches(&dir);
    assert!(
        branches.contains(&"feature/done".to_string()),
        "an ignored branch must survive an auto-confirmed clean, got {branches:?}"
    );
}

#[test]
fn per_branch_ignore_flag_survives_clean() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "ignore", "feature/done"])
        .current_dir(dir.path())
        .assert()
        .success();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success();

    assert!(git_branches(&dir).contains(&"feature/done".to_string()));
}

#[test]
fn config_add_ignore_and_list() {
    let dir = init_repo();
    configure(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-ignore", "wip/*"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Added ignore pattern: wip/*"));

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("ignore:"))
        .stderr(predicate::str::contains("wip/*"));

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "remove-ignore", "wip/*"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Removed ignore pattern: wip/*"));

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("wip/*").not());
}

#[test]
fn config_ignore_and_unignore_individual_branch() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "ignore", "feature/done"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("is now ignored"));

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("branch ignored: feature/done"));

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "unignore", "feature/done"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("no longer ignored"));

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("branch ignored: (none)"));

    // Once unignored, the merged branch is a deletion candidate again.
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["-y", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success();
    assert!(!git_branches(&dir).contains(&"feature/done".to_string()));
}

#[test]
fn config_remove_ignore_keeps_the_other_patterns() {
    let dir = init_repo();
    configure(&dir);

    for pattern in ["wip/*", "scratch", "tmp/*"] {
        Command::cargo_bin("git-sync")
            .unwrap()
            .args(["config", "add-ignore", pattern])
            .current_dir(dir.path())
            .assert()
            .success();
    }

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "remove-ignore", "scratch"])
        .current_dir(dir.path())
        .assert()
        .success();

    let output = StdCommand::new("git")
        .args(["config", "--get-all", "sync.ignore"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let patterns: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect();
    assert_eq!(
        patterns,
        vec!["wip/*".to_string(), "tmp/*".to_string()],
        "remove-ignore must preserve the remaining patterns and their order"
    );
}

#[test]
fn config_list_renders_empty_sections_as_none() {
    let dir = init_repo();

    // A config section that exists but holds no protected pattern.
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "add-ignore", "wip/*"])
        .current_dir(dir.path())
        .assert()
        .success();

    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["config", "list"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("protected: (none)"))
        .stderr(predicate::str::contains("ignore: wip/*"))
        .stderr(predicate::str::contains("branch ignored: (none)"));
}

// ── JSON output ──────────────────────────────────────────────────────

/// Run the binary and parse its stdout as the single JSON document it must be.
fn json_output(dir: &TempDir, args: &[&str]) -> serde_json::Value {
    json_output_at(dir.path(), args)
}

fn json_output_at(path: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let output = Command::cargo_bin("git-sync")
        .unwrap()
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.lines().count(),
        1,
        "piped output must be a single compact JSON line, got: {stdout}"
    );
    serde_json::from_str(&stdout).expect("stdout must be valid JSON")
}

/// Remote detection runs the same strategies as local detection (#28): a
/// squash-merged remote branch is reported at the default effort, but not at
/// `--effort 1`.
#[test]
fn json_reports_squash_merged_remote_branches() {
    let (_dir, work, _bare) = init_repo_with_remote();

    StdCommand::new("git")
        .args(["checkout", "-b", "feature/squashed", "main"])
        .current_dir(&work)
        .output()
        .unwrap();
    std::fs::write(work.join("squashed.txt"), "squashed").unwrap();
    StdCommand::new("git")
        .args(["add", "."])
        .current_dir(&work)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "squashed feature"])
        .current_dir(&work)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push", "-u", "origin", "feature/squashed"])
        .current_dir(&work)
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["checkout", "main"])
        .current_dir(&work)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["merge", "--squash", "feature/squashed"])
        .current_dir(&work)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["commit", "-m", "squash merge"])
        .current_dir(&work)
        .output()
        .unwrap();
    StdCommand::new("git")
        .args(["push", "origin", "main"])
        .current_dir(&work)
        .output()
        .unwrap();

    let doc = json_output_at(&work, &["--json", "--dry-run", "--no-fetch"]);
    assert_eq!(doc["remotes"][0]["remote"], "origin");
    let merged = doc["remotes"][0]["merged"].as_array().unwrap();
    assert!(
        merged.iter().any(|b| b == "feature/squashed"),
        "default effort should report the squash-merged remote branch, got {merged:?}"
    );

    let quick = json_output_at(
        &work,
        &["--json", "--dry-run", "--no-fetch", "--effort", "1"],
    );
    let quick_merged = quick["remotes"][0]["merged"].as_array().unwrap();
    assert!(
        !quick_merged.iter().any(|b| b == "feature/squashed"),
        "quick effort keeps remote detection on ancestor merges, got {quick_merged:?}"
    );
}

#[test]
fn json_dry_run_reports_candidates_without_deleting() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    let doc = json_output(&dir, &["--json", "--dry-run", "--no-fetch"]);

    assert_eq!(doc["version"], 1);
    assert_eq!(doc["status"], "success");
    assert_eq!(doc["dry_run"], true);
    assert_eq!(doc["effort"], 2, "the default effort level is reported");
    assert_eq!(doc["local"]["merged"][0], "feature/done");
    assert_eq!(doc["local"]["branches"][0]["branch"], "feature/done");
    assert_eq!(doc["local"]["branches"][0]["reason"], "merged");
    assert_eq!(doc["local"]["branches"][0]["selected"], true);
    assert_eq!(doc["local"]["branches"][0]["status"], "dry_run");
    assert_eq!(doc["summary"]["local_branches_deleted"], 0);
    assert_eq!(doc["errors"].as_array().unwrap().len(), 0);

    // Nothing was touched.
    let branches = git_branches(&dir);
    assert!(branches.contains(&"feature/done".to_string()));
}

#[test]
fn json_run_reports_deleted_branches() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    let doc = json_output(&dir, &["--json", "--no-fetch"]);

    assert_eq!(doc["status"], "success");
    assert_eq!(doc["dry_run"], false);
    assert_eq!(doc["summary"]["local_branches_deleted"], 1);
    assert_eq!(doc["local"]["branches"][0]["branch"], "feature/done");
    assert_eq!(doc["local"]["branches"][0]["status"], "deleted");

    let branches = git_branches(&dir);
    assert!(!branches.contains(&"feature/done".to_string()));
    assert!(branches.contains(&"feature/wip".to_string()));
}

#[test]
fn json_implies_yes_without_prompting() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    // No `-y`: the run must still complete non-interactively.
    Command::cargo_bin("git-sync")
        .unwrap()
        .args(["--json", "--no-fetch"])
        .current_dir(dir.path())
        .assert()
        .success();

    assert!(!git_branches(&dir).contains(&"feature/done".to_string()));
}

#[test]
fn json_keeps_stdout_free_of_human_output() {
    let dir = init_repo();
    configure(&dir);
    add_branches(&dir);

    let output = Command::cargo_bin("git-sync")
        .unwrap()
        .args(["--json", "--dry-run", "--no-fetch"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(
        output.stderr.is_empty(),
        "stderr must stay silent in JSON mode"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with('{') && stdout.trim_end().ends_with('}'));
}

#[test]
fn json_reports_a_young_worktree_as_too_young() {
    let dir = init_repo();
    configure(&dir);
    let wt_path = add_merged_worktree(&dir, "feature/young", "wt-young");

    let doc = json_output(
        &dir,
        &["--json", "--no-fetch", "--local-only", "--min-age", "1h"],
    );

    assert_eq!(doc["min_age"], "1h");
    // Matched on the branch, not the path: macOS resolves the temp dir through
    // /private, so git reports a different string than the fixture holds.
    let worktrees = doc["local"]["worktrees"].as_array().unwrap();
    let entry = worktrees
        .iter()
        .find(|w| w["branch"] == "feature/young")
        .expect("the young worktree should be reported");
    assert_eq!(entry["status"], "too_young");
    assert_eq!(entry["kind"], "branch");
    assert_eq!(doc["summary"]["worktrees_removed"], 0);
    assert!(wt_path.exists());
}

#[test]
fn json_reports_fatal_error_outside_a_repository() {
    let dir = tempfile::tempdir().unwrap();

    let output = Command::cargo_bin("git-sync")
        .unwrap()
        .args(["--json", "--no-fetch"])
        .current_dir(dir.path())
        .env("GIT_CEILING_DIRECTORIES", dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success(), "a fatal error must exit non-zero");
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["status"], "error");
    assert_eq!(doc["errors"][0]["kind"], "other");
    assert!(
        doc["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Not a git repository")
    );
}

#[test]
fn json_requires_an_existing_configuration() {
    let dir = init_repo();
    // No `[sync]` section: the wizard cannot run without a human.

    let output = Command::cargo_bin("git-sync")
        .unwrap()
        .args(["--json", "--no-fetch"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    assert!(!output.status.success());
    let doc: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(doc["status"], "error");
    assert!(
        doc["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("not configured")
    );
}

#[test]
fn config_list_json_reports_values() {
    let dir = init_repo();
    configure(&dir);

    let doc = json_output(&dir, &["config", "list", "--json"]);

    assert_eq!(doc["configured"], true);
    assert_eq!(doc["protected"][0], "main");
    assert_eq!(doc["ignore"].as_array().unwrap().len(), 0);
    assert!(
        doc["remotes"].is_null(),
        "no configured remote means all remotes"
    );
    assert!(
        doc["worktrunk"].is_null(),
        "unset worktrunk means auto-detect"
    );
}

#[test]
fn config_list_json_reports_unconfigured_repository() {
    let dir = init_repo();

    let doc = json_output(&dir, &["config", "list", "--json"]);

    assert_eq!(doc["configured"], false);
    assert_eq!(doc["protected"].as_array().unwrap().len(), 0);
}
