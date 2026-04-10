use anyhow::Result;

use crate::branches::{find_merged_local, find_merged_remote};
use crate::config::Config;
use crate::git::{Git, Worktree};
use crate::ui::Ui;
use crate::worktrees::{find_orphan_worktrees, find_worktrees_for_branches};

/// Options controlling cleaner behaviour, derived from CLI flags.
pub struct CleanerOptions {
    pub yes: bool,
    pub dry_run: bool,
    pub no_fetch: bool,
    pub local_only: bool,
    pub remote_only: bool,
    pub no_worktrees: bool,
    pub use_worktrunk: bool,
}

/// Run the full clean-up workflow.
pub fn run(git: &Git, config: &Config, ui: &Ui, opts: &CleanerOptions) -> Result<()> {
    // ── 1. Fetch & prune ─────────────────────────────────────────────

    if !opts.no_fetch {
        let remotes = effective_remotes(git, config)?;
        if !remotes.is_empty() {
            ui.heading(&format!(
                "Fetching and pruning {} remote(s):",
                remotes.len()
            ));
            ui.bullet_list(&remotes);
            if opts.dry_run {
                ui.muted("  (dry-run) Skipping remote update.");
            } else {
                git.remote_update_prune()?;
                ui.success("Remotes updated.");
            }
        }
    }

    let mut total_deleted = 0usize;

    // ── 3. Local branches ────────────────────────────────────────────

    if !opts.remote_only {
        let merged = find_merged_local(git, config)?;

        if merged.is_empty() {
            ui.muted("No merged local branches to delete.");
        } else {
            ui.heading(&format!("Found {} merged local branch(es):", merged.len()));
            ui.bullet_list(&merged);

            let to_delete = if opts.yes {
                merged.clone()
            } else {
                let defaults: Vec<bool> = vec![true; merged.len()];
                ui.multi_select("Select branches to delete", &merged, &merged, &defaults)?
            };

            if !to_delete.is_empty() {
                // Remove worktrees for these branches first
                if !opts.no_worktrees {
                    remove_worktrees_for_branches(git, ui, opts, &to_delete)?;
                }

                for branch in &to_delete {
                    if opts.dry_run {
                        ui.muted(&format!(
                            "  (dry-run) Would delete local branch '{branch}'."
                        ));
                    } else {
                        match git.branch_delete(branch) {
                            Ok(()) => total_deleted += 1,
                            Err(e) => ui.warning(&format!("  Failed to delete '{branch}': {e}")),
                        }
                    }
                }
                if !opts.dry_run {
                    ui.summary(total_deleted, "local branch", "local branches", "deleted");
                }
            }
        }
    }

    // ── 4. Remote branches ───────────────────────────────────────────

    if !opts.local_only {
        let remotes = effective_remotes(git, config)?;

        for remote in &remotes {
            let merged = find_merged_remote(git, config, remote)?;

            if merged.is_empty() {
                ui.muted(&format!("No merged remote branches on '{remote}'."));
                continue;
            }

            let display: Vec<String> = merged.iter().map(|b| format!("{remote}/{b}")).collect();
            ui.heading(&format!(
                "Found {} merged remote branch(es) on '{remote}':",
                merged.len()
            ));
            ui.bullet_list(&display);

            let to_delete = if opts.yes {
                merged.clone()
            } else {
                let defaults: Vec<bool> = vec![true; merged.len()];
                ui.multi_select("Select branches to delete", &merged, &display, &defaults)?
            };

            let mut remote_deleted = 0usize;
            for branch in &to_delete {
                if opts.dry_run {
                    ui.muted(&format!("  (dry-run) Would delete '{remote}/{branch}'."));
                } else {
                    match git.push_delete(remote, branch) {
                        Ok(()) => remote_deleted += 1,
                        Err(e) => {
                            ui.warning(&format!("  Failed to delete '{remote}/{branch}': {e}"));
                        }
                    }
                }
            }
            if !opts.dry_run && remote_deleted > 0 {
                ui.summary(
                    remote_deleted,
                    "remote branch",
                    "remote branches",
                    "deleted",
                );
            }
        }
    }

    // ── 5. Orphan worktrees ──────────────────────────────────────────

    if !opts.no_worktrees {
        let orphans = find_orphan_worktrees(git)?;

        if orphans.is_empty() {
            ui.muted("No orphan worktrees to remove.");
        } else {
            let display: Vec<String> = orphans
                .iter()
                .map(|wt| {
                    format!(
                        "{} (branch: {})",
                        wt.path,
                        wt.branch.as_deref().unwrap_or("detached")
                    )
                })
                .collect();
            ui.heading(&format!("Found {} orphan worktree(s):", orphans.len()));
            ui.bullet_list(&display);

            if opts.yes || ui.confirm("Remove orphan worktrees?", false)? {
                let mut removed = 0usize;
                for wt in &orphans {
                    if opts.dry_run {
                        ui.muted(&format!("  (dry-run) Would remove worktree '{}'.", wt.path));
                    } else {
                        match remove_worktree(git, wt, opts.use_worktrunk) {
                            Ok(()) => removed += 1,
                            Err(e) => {
                                ui.warning(&format!("  Failed to remove '{}': {e}", wt.path));
                            }
                        }
                    }
                }
                if !opts.dry_run && removed > 0 {
                    ui.summary(removed, "worktree", "worktrees", "removed");
                }
            }
        }
    }

    // ── Done ─────────────────────────────────────────────────────────

    ui.blank();
    if opts.dry_run {
        ui.muted("Dry run complete. No changes were made.");
    } else {
        ui.success("Done.");
    }

    Ok(())
}

/// Remove worktrees that are associated with branches about to be deleted.
fn remove_worktrees_for_branches(
    git: &Git,
    ui: &Ui,
    opts: &CleanerOptions,
    branches: &[String],
) -> Result<()> {
    let worktrees = find_worktrees_for_branches(git, branches)?;

    if worktrees.is_empty() {
        return Ok(());
    }

    let display: Vec<String> = worktrees
        .iter()
        .map(|wt| {
            format!(
                "{} (branch: {})",
                wt.path,
                wt.branch.as_deref().unwrap_or("?")
            )
        })
        .collect();
    ui.heading("Worktrees for branches about to be deleted:");
    ui.bullet_list(&display);

    if opts.yes || ui.confirm("Remove these worktrees first?", true)? {
        for wt in &worktrees {
            if opts.dry_run {
                ui.muted(&format!("  (dry-run) Would remove worktree '{}'.", wt.path));
            } else {
                match remove_worktree(git, wt, opts.use_worktrunk) {
                    Ok(()) => {}
                    Err(e) => {
                        ui.warning(&format!("  Failed to remove '{}': {e}", wt.path));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Determine which remotes to operate on.
fn effective_remotes(git: &Git, config: &Config) -> Result<Vec<String>> {
    match &config.remotes {
        Some(configured) => Ok(configured.clone()),
        None => git.remotes(),
    }
}

/// Remove a single worktree, optionally using worktrunk to trigger hooks.
fn remove_worktree(git: &Git, wt: &Worktree, use_worktrunk: bool) -> Result<()> {
    if use_worktrunk {
        match &wt.branch {
            Some(branch) => git.worktrunk_remove(branch),
            None => git.worktrunk_remove_by_path(&wt.path),
        }
    } else {
        git.worktree_remove(&wt.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    /// Create a temporary git repo with an initial commit on `main`,
    /// a merged branch `feature/done`, and an unmerged branch `feature/wip`.
    fn init_repo_with_branches() -> (tempfile::TempDir, Git) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        StdCommand::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();

        std::fs::write(path.join("README.md"), "# test").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();

        // Create and merge a feature branch
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/done"])
            .current_dir(path)
            .output()
            .unwrap();
        std::fs::write(path.join("done.txt"), "done").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "feature done"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["merge", "feature/done"])
            .current_dir(path)
            .output()
            .unwrap();

        // Create an unmerged branch
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/wip"])
            .current_dir(path)
            .output()
            .unwrap();
        std::fs::write(path.join("wip.txt"), "wip").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "work in progress"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()
            .unwrap();

        let git = Git::with_workdir(false, path);
        (dir, git)
    }

    fn default_config() -> Config {
        Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        }
    }

    fn opts_yes_skip_network() -> CleanerOptions {
        CleanerOptions {
            yes: true,
            dry_run: false,
            no_fetch: true,
            local_only: false,
            remote_only: false,
            no_worktrees: false,
            use_worktrunk: false,
        }
    }

    #[test]
    fn test_run_deletes_merged_local_branches() {
        let (_dir, git) = init_repo_with_branches();
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        let branches_before = git.local_branches().unwrap();
        assert!(branches_before.contains(&"feature/done".to_string()));
        assert!(branches_before.contains(&"feature/wip".to_string()));

        run(&git, &config, &ui, &opts).unwrap();

        let branches_after = git.local_branches().unwrap();
        assert!(!branches_after.contains(&"feature/done".to_string()));
        assert!(branches_after.contains(&"feature/wip".to_string()));
        assert!(branches_after.contains(&"main".to_string()));
    }

    #[test]
    fn test_run_dry_run_preserves_branches() {
        let (_dir, git) = init_repo_with_branches();
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.dry_run = true;

        let branches_before = git.local_branches().unwrap();

        run(&git, &config, &ui, &opts).unwrap();

        let branches_after = git.local_branches().unwrap();
        assert_eq!(branches_before, branches_after);
    }

    #[test]
    fn test_run_no_merged_branches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        StdCommand::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();
        std::fs::write(path.join("README.md"), "# test").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts).unwrap();
    }

    #[test]
    fn test_run_remote_only_skips_local_deletion() {
        let (_dir, git) = init_repo_with_branches();
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.remote_only = true;

        run(&git, &config, &ui, &opts).unwrap();

        let branches = git.local_branches().unwrap();
        assert!(branches.contains(&"feature/done".to_string()));
    }

    #[test]
    fn test_run_local_only_skips_remote_deletion() {
        let (_dir, git) = init_repo_with_branches();
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.local_only = true;

        run(&git, &config, &ui, &opts).unwrap();

        let branches = git.local_branches().unwrap();
        assert!(!branches.contains(&"feature/done".to_string()));
    }

    #[test]
    fn test_run_no_worktrees_skips_worktree_cleanup() {
        let (_dir, git) = init_repo_with_branches();
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.no_worktrees = true;

        run(&git, &config, &ui, &opts).unwrap();

        let branches = git.local_branches().unwrap();
        assert!(!branches.contains(&"feature/done".to_string()));
    }

    #[test]
    fn test_effective_remotes_uses_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        StdCommand::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();
        std::fs::write(path.join("README.md"), "# test").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();

        let git = Git::with_workdir(false, path);

        let config_with = Config {
            protected: vec!["main".to_string()],
            remotes: Some(vec!["origin".to_string(), "upstream".to_string()]),
            worktrunk: None,
        };
        let remotes = effective_remotes(&git, &config_with).unwrap();
        assert_eq!(remotes, vec!["origin", "upstream"]);

        let config_without = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let remotes = effective_remotes(&git, &config_without).unwrap();
        assert!(remotes.is_empty());
    }

    #[test]
    fn test_run_with_worktree_for_merged_branch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();

        StdCommand::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();

        std::fs::write(path.join("README.md"), "# test").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();

        // Create and merge a branch
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/wt-test"])
            .current_dir(path)
            .output()
            .unwrap();
        std::fs::write(path.join("wt.txt"), "worktree test").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "worktree feature"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["merge", "feature/wt-test"])
            .current_dir(path)
            .output()
            .unwrap();

        // Create a worktree for the merged branch
        let wt_path = path.join("wt-feature");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                wt_path.to_str().unwrap(),
                "feature/wt-test",
            ])
            .current_dir(path)
            .output()
            .unwrap();

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts).unwrap();

        let branches = git.local_branches().unwrap();
        assert!(!branches.contains(&"feature/wt-test".to_string()));
        assert!(!wt_path.exists());
    }
}
