use std::collections::HashMap;

use anyhow::Result;

use crate::branches::{find_merged_local, find_merged_remote, resolve_merge_targets};
use crate::config::Config;
use crate::git::{Git, Worktree};
use crate::ui::Ui;
use crate::worktrees::find_orphan_worktrees;

/// Return a display-friendly path with `$HOME` replaced by `~`.
fn tilde_path(abs: &str) -> String {
    if let Ok(home) = std::env::var("HOME")
        && let Some(rest) = abs.strip_prefix(&home)
    {
        return format!("~{rest}");
    }
    abs.to_string()
}

/// Options controlling cleaner behaviour, derived from CLI flags.
#[derive(Debug, Clone, Default)]
pub struct CleanerOptions {
    pub yes: bool,
    pub dry_run: bool,
    pub no_fetch: bool,
    pub no_pull: bool,
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

    // ── 2. Pull / fast-forward target branches ─────────────────────

    if !opts.no_pull {
        let targets = resolve_merge_targets(git, config)?;
        if !targets.is_empty() {
            let current = git.current_branch()?;
            let worktrees = git.worktree_list()?;

            // Map branch name → worktree path for branches checked out somewhere.
            let wt_map: HashMap<String, String> = worktrees
                .iter()
                .filter(|wt| !wt.is_bare)
                .filter_map(|wt| wt.branch.as_ref().map(|b| (b.clone(), wt.path.clone())))
                .collect();

            // Collect targets that have upstream tracking info.
            let mut pullable: Vec<(String, String, String)> = Vec::new(); // (branch, remote, upstream_branch)
            for target in &targets {
                if let Some((remote, upstream_branch)) = git.branch_upstream(target)? {
                    pullable.push((target.clone(), remote, upstream_branch));
                }
            }

            if pullable.is_empty() {
                ui.muted("No target branches with upstream tracking to pull.");
            } else {
                let display: Vec<String> = pullable
                    .iter()
                    .map(|(branch, remote, _)| format!("{branch} (from {remote})"))
                    .collect();
                ui.heading(&format!("Pulling {} target branch(es):", pullable.len()));
                ui.bullet_list(&display);

                for (branch, remote, upstream_branch) in &pullable {
                    if opts.dry_run {
                        ui.muted(&format!(
                            "  (dry-run) Would pull '{branch}' from {remote}/{upstream_branch}."
                        ));
                        continue;
                    }

                    let result = if *branch == current {
                        // Checked out in the current working directory
                        git.pull_ff_only()
                    } else if let Some(wt_path) = wt_map.get(branch) {
                        // Checked out in another worktree
                        git.pull_ff_only_in(wt_path)
                    } else {
                        // Not checked out anywhere — fast-forward via fetch
                        git.fetch_update_branch(remote, upstream_branch, branch)
                    };

                    match result {
                        Ok(()) => {
                            ui.success(&format!("{} updated.", console::style(&branch).cyan()))
                        }
                        Err(e) => ui.error(&format!("{}: {}", console::style(&branch).red(), e)),
                    }
                }
            }
        }
    }

    let mut total_deleted = 0usize;

    // ── 3. Local branches & worktrees ────────────────────────────────
    //
    // Merged local branches, their associated worktrees, and orphan
    // worktrees are presented in a single unified multiselect so the
    // user confirms everything in one pass.

    if !opts.remote_only {
        let merged = find_merged_local(git, config)?;

        // Build a map of branch → worktree for branches that have one.
        let worktrees = git.worktree_list()?;
        let wt_map: HashMap<String, Worktree> = worktrees
            .iter()
            .filter(|wt| !wt.is_bare)
            .filter_map(|wt| wt.branch.as_ref().map(|b| (b.clone(), (*wt).clone())))
            .collect();

        // Collect orphan worktrees (if worktree cleanup is enabled).
        let (orphan_locked, orphan_unlocked) = if !opts.no_worktrees {
            let orphans = find_orphan_worktrees(git)?;
            let (locked, unlocked): (Vec<_>, Vec<_>) =
                orphans.into_iter().partition(|wt| wt.is_locked);
            (locked, unlocked)
        } else {
            (Vec::new(), Vec::new())
        };

        // Report locked worktrees (both branch-associated and orphan).
        if !opts.no_worktrees {
            for branch in &merged {
                if let Some(wt) = wt_map.get(branch)
                    && wt.is_locked
                {
                    ui.muted(&format_locked_skip_message(wt));
                }
            }
            for wt in &orphan_locked {
                ui.muted(&format_locked_skip_message(wt));
            }
        }

        let has_merged = !merged.is_empty();
        let has_orphans = !orphan_unlocked.is_empty();

        if !has_merged && !has_orphans {
            ui.muted("No merged local branches to delete.");
            if !opts.no_worktrees {
                ui.muted("No orphan worktrees to remove.");
            }
        } else {
            // --- Build the unified multiselect ---

            let mut values: Vec<String> = Vec::new();
            let mut labels: Vec<String> = Vec::new();
            let mut defaults: Vec<bool> = Vec::new();
            let mut hints: Vec<String> = Vec::new();

            // Merged branches (with worktree path shown in label if applicable).
            for branch in &merged {
                values.push(format!("branch:{branch}"));
                let has_unlocked_wt = wt_map.get(branch).is_some_and(|wt| !wt.is_locked);
                if !opts.no_worktrees && has_unlocked_wt {
                    let wt = &wt_map[branch];
                    labels.push(format!(
                        "{branch} {}",
                        console::style(format!("({})", tilde_path(&wt.path)))
                            .dim()
                            .italic()
                    ));
                    hints.push("branch + worktree".to_string());
                } else {
                    labels.push(branch.clone());
                    hints.push(String::new());
                }
                defaults.push(true);
            }

            // Orphan worktrees.
            for wt in &orphan_unlocked {
                values.push(format!("orphan-wt:{}", wt.path));
                labels.push(tilde_path(&wt.path));
                hints.push(format!(
                    "orphan worktree, branch: {}",
                    wt.branch.as_deref().unwrap_or("detached")
                ));
                defaults.push(false);
            }

            // Heading.
            let heading = match (has_merged, has_orphans) {
                (true, true) => format!(
                    "Found {} merged local branch(es) and {} orphan worktree(s):",
                    merged.len(),
                    orphan_unlocked.len()
                ),
                (true, false) => {
                    format!("Found {} merged local branch(es):", merged.len())
                }
                (false, true) => format!("Found {} orphan worktree(s):", orphan_unlocked.len()),
                _ => unreachable!(),
            };
            ui.heading(&heading);

            let prompt = if has_merged && has_orphans {
                "Select branches and worktrees to delete"
            } else if has_orphans {
                "Select orphan worktrees to remove"
            } else {
                "Select branches to delete"
            };

            let selected = if opts.yes {
                values.clone()
            } else {
                ui.multi_select(prompt, &values, &labels, &defaults, &hints)?
            };

            // --- Process the selections ---

            // 1. Remove worktrees for selected merged branches first.
            if !opts.no_worktrees {
                let mut wt_removed = 0usize;
                for branch in &merged {
                    let key = format!("branch:{branch}");
                    if !selected.contains(&key) {
                        continue;
                    }
                    if let Some(wt) = wt_map.get(branch) {
                        if wt.is_locked {
                            continue; // already reported above
                        }
                        if opts.dry_run {
                            ui.muted(&format!("  (dry-run) Would remove worktree '{}'.", wt.path));
                        } else {
                            match remove_worktree(git, wt, opts.use_worktrunk, false) {
                                Ok(()) => {
                                    wt_removed += 1;
                                    ui.success(&format!(
                                        "{} removed.",
                                        console::style(tilde_path(&wt.path)).cyan(),
                                    ));
                                }
                                Err(e) => {
                                    ui.error(&format!(
                                        "Failed to remove '{}': {e}",
                                        console::style(tilde_path(&wt.path)).red()
                                    ));
                                }
                            }
                        }
                    }
                }

                // 2. Remove selected orphan worktrees.
                for wt in &orphan_unlocked {
                    let key = format!("orphan-wt:{}", wt.path);
                    if !selected.contains(&key) {
                        continue;
                    }
                    if opts.dry_run {
                        ui.muted(&format!("  (dry-run) Would remove worktree '{}'.", wt.path));
                    } else {
                        match remove_worktree(git, wt, opts.use_worktrunk, true) {
                            Ok(()) => {
                                wt_removed += 1;
                                ui.success(&format!(
                                    "{} removed.",
                                    console::style(tilde_path(&wt.path)).cyan(),
                                ));
                            }
                            Err(e) => {
                                ui.error(&format!(
                                    "Failed to remove '{}': {e}",
                                    console::style(tilde_path(&wt.path)).red()
                                ));
                            }
                        }
                    }
                }
                if !opts.dry_run && wt_removed > 0 {
                    ui.summary(wt_removed, "worktree", "worktrees", "removed");
                }
            }

            // 3. Delete selected merged branches.
            for branch in &merged {
                let key = format!("branch:{branch}");
                if !selected.contains(&key) {
                    continue;
                }
                if opts.dry_run {
                    ui.muted(&format!(
                        "  (dry-run) Would delete local branch '{branch}'."
                    ));
                } else {
                    match git.branch_delete(branch) {
                        Ok(()) => total_deleted += 1,
                        Err(e) => ui.error(&format!(
                            "Failed to delete '{}': {e}",
                            console::style(&branch).red()
                        )),
                    }
                }
            }
            if !opts.dry_run && total_deleted > 0 {
                ui.summary(total_deleted, "local branch", "local branches", "deleted");
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
                ui.multi_select(
                    "Select branches to delete",
                    &merged,
                    &display,
                    &defaults,
                    &[],
                )?
            };

            let mut remote_deleted = 0usize;
            for branch in &to_delete {
                if opts.dry_run {
                    ui.muted(&format!("  (dry-run) Would delete '{remote}/{branch}'."));
                } else {
                    match git.push_delete(remote, branch) {
                        Ok(()) => remote_deleted += 1,
                        Err(e) => {
                            ui.error(&format!("  Failed to delete '{remote}/{branch}': {e}"));
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

    // ── Done ─────────────────────────────────────────────────────────

    ui.blank();
    if opts.dry_run {
        ui.muted("Dry run complete. No changes were made.");
    } else {
        ui.success("Done.");
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

/// Format an informational skip message for a locked worktree.
fn format_locked_skip_message(wt: &Worktree) -> String {
    let branch_label = wt.branch.as_deref().unwrap_or("detached");
    match &wt.lock_reason {
        Some(reason) => {
            format!(
                "  Skipping locked worktree '{}' (branch: {branch_label}): {reason}",
                wt.path
            )
        }
        None => {
            format!(
                "  Skipping locked worktree '{}' (branch: {branch_label}).",
                wt.path
            )
        }
    }
}

/// Remove a single worktree, optionally using worktrunk to trigger hooks.
///
/// When `force` is true, passes `--force` to `git worktree remove`.
/// This is needed for orphan worktrees whose branch ref has been deleted,
/// since Git always considers them dirty without a branch to compare against.
fn remove_worktree(git: &Git, wt: &Worktree, use_worktrunk: bool, force: bool) -> Result<()> {
    if use_worktrunk {
        match &wt.branch {
            Some(branch) => git.worktrunk_remove(branch),
            None => git.worktrunk_remove_by_path(&wt.path),
        }
    } else {
        git.worktree_remove(&wt.path, force)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

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
            no_pull: true,
            local_only: false,
            remote_only: false,
            no_worktrees: false,
            use_worktrunk: false,
        }
    }

    #[test]
    fn test_run_deletes_merged_local_branches() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        let branches_before = git.local_branches()?;
        assert!(branches_before.contains(&"feature/done".to_string()));
        assert!(branches_before.contains(&"feature/wip".to_string()));

        run(&git, &config, &ui, &opts)?;

        let branches_after = git.local_branches()?;
        assert!(!branches_after.contains(&"feature/done".to_string()));
        assert!(branches_after.contains(&"feature/wip".to_string()));
        assert!(branches_after.contains(&"main".to_string()));
        Ok(())
    }

    #[test]
    fn test_run_dry_run_preserves_branches() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.dry_run = true;

        let branches_before = git.local_branches()?;

        run(&git, &config, &ui, &opts)?;

        let branches_after = git.local_branches()?;
        assert_eq!(branches_before, branches_after);
        Ok(())
    }

    #[test]
    fn test_run_no_merged_branches() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts)?;
        Ok(())
    }

    #[test]
    fn test_run_remote_only_skips_local_deletion() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.remote_only = true;

        run(&git, &config, &ui, &opts)?;

        let branches = git.local_branches()?;
        assert!(branches.contains(&"feature/done".to_string()));
        Ok(())
    }

    #[test]
    fn test_run_local_only_skips_remote_deletion() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.local_only = true;

        run(&git, &config, &ui, &opts)?;

        let branches = git.local_branches()?;
        assert!(!branches.contains(&"feature/done".to_string()));
        Ok(())
    }

    #[test]
    fn test_run_no_worktrees_skips_worktree_cleanup() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.no_worktrees = true;

        run(&git, &config, &ui, &opts)?;

        let branches = git.local_branches()?;
        assert!(!branches.contains(&"feature/done".to_string()));
        Ok(())
    }

    #[test]
    fn test_effective_remotes_uses_config() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config_with = Config {
            protected: vec!["main".to_string()],
            remotes: Some(vec!["origin".to_string(), "upstream".to_string()]),
            worktrunk: None,
        };
        let remotes = effective_remotes(&git, &config_with)?;
        assert_eq!(remotes, vec!["origin", "upstream"]);

        let config_without = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let remotes = effective_remotes(&git, &config_without)?;
        assert!(remotes.is_empty());
        Ok(())
    }

    #[test]
    fn test_run_with_worktree_for_merged_branch() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create and merge a branch
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/wt-test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("wt.txt"), "worktree test")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "worktree feature"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "feature/wt-test"])
            .current_dir(path)
            .output()?;

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
            .output()?;

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts)?;

        let branches = git.local_branches()?;
        assert!(!branches.contains(&"feature/wt-test".to_string()));
        assert!(!wt_path.exists());
        Ok(())
    }

    #[test]
    fn test_run_skips_locked_worktree() -> Result<()> {
        let (_dir, git, wt_path) = crate::test_helpers::init_repo_with_locked_worktree()?;
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts)?;

        // The locked worktree directory should still exist
        assert!(
            std::path::Path::new(&wt_path).exists(),
            "locked worktree should not be removed"
        );

        // The branch cannot be deleted because it's still checked out
        // in the locked worktree — git refuses to delete it. This is
        // expected: the worktree removal was skipped, so the branch
        // deletion also fails gracefully (logged as a warning).
        let branches = git.local_branches()?;
        assert!(
            branches.contains(&"feature/locked-wt".to_string()),
            "branch should survive because its locked worktree prevents deletion"
        );
        Ok(())
    }

    #[test]
    fn test_format_locked_skip_message_no_reason() {
        let wt = Worktree {
            path: "/tmp/wt".to_string(),
            head: None,
            branch: Some("feature/x".to_string()),
            is_bare: false,
            is_locked: true,
            lock_reason: None,
        };
        let msg = format_locked_skip_message(&wt);
        assert!(msg.contains("Skipping locked worktree"));
        assert!(msg.contains("/tmp/wt"));
        assert!(msg.contains("feature/x"));
    }

    #[test]
    fn test_format_locked_skip_message_with_reason() {
        let wt = Worktree {
            path: "/tmp/wt".to_string(),
            head: None,
            branch: Some("feature/x".to_string()),
            is_bare: false,
            is_locked: true,
            lock_reason: Some("do not touch".to_string()),
        };
        let msg = format_locked_skip_message(&wt);
        assert!(msg.contains("Skipping locked worktree"));
        assert!(msg.contains("/tmp/wt"));
        assert!(msg.contains("feature/x"));
        assert!(msg.contains("do not touch"));
    }

    #[test]
    fn test_run_handles_orphan_worktrees() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a branch and a worktree for it
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/orphan"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("orphan.txt"), "orphan")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "orphan feature"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "feature/orphan"])
            .current_dir(path)
            .output()?;

        let wt_path = path.join("wt-orphan");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                wt_path.to_str().unwrap(),
                "feature/orphan",
            ])
            .current_dir(path)
            .output()?;

        // Delete the branch ref, making the worktree orphaned
        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/orphan"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        // The run should complete without error — even though `git worktree
        // remove` fails (the worktree appears dirty after its branch ref is
        // deleted), the error is caught and logged as a warning.
        run(&git, &config, &ui, &opts)?;
        Ok(())
    }

    #[test]
    fn test_run_removes_clean_orphan_worktree() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a branch at the same commit as main (no diverging content)
        StdCommand::new("git")
            .args(["branch", "feature/clean-orphan"])
            .current_dir(path)
            .output()?;

        let wt_path = path.join("wt-clean-orphan");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                wt_path.to_str().unwrap(),
                "feature/clean-orphan",
            ])
            .current_dir(path)
            .output()?;

        // Delete the branch ref, making the worktree orphaned.
        // Since the worktree content matches the commit, it is clean
        // and `git worktree remove` will succeed.
        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/clean-orphan"])
            .current_dir(path)
            .output()?;

        assert!(wt_path.exists(), "worktree should exist before cleanup");

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts)?;

        assert!(!wt_path.exists(), "clean orphan worktree should be removed");
        Ok(())
    }

    #[test]
    fn test_run_skips_locked_orphan_worktree() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a branch and worktree, then merge the branch
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/locked-orphan"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("locked-orphan.txt"), "locked orphan")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "locked orphan feature"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "feature/locked-orphan"])
            .current_dir(path)
            .output()?;

        let wt_path = path.join("wt-locked-orphan");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                wt_path.to_str().unwrap(),
                "feature/locked-orphan",
            ])
            .current_dir(path)
            .output()?;

        // Lock the worktree
        StdCommand::new("git")
            .args(["worktree", "lock", wt_path.to_str().unwrap()])
            .current_dir(path)
            .output()?;

        // Delete the branch ref, making the worktree orphaned
        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/locked-orphan"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts)?;

        // The locked orphan worktree should still exist
        assert!(
            wt_path.exists(),
            "locked orphan worktree should not be removed"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn test_tilde_path_replaces_home() {
        let home = std::env::var("HOME").expect("HOME must be set for this test");
        assert_eq!(
            tilde_path(&format!("{home}/projects/repo")),
            "~/projects/repo"
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_tilde_path_preserves_non_home_path() {
        assert_eq!(tilde_path("/tmp/some/path"), "/tmp/some/path");
    }

    #[test]
    #[cfg(unix)]
    fn test_tilde_path_exact_home() {
        let home = std::env::var("HOME").expect("HOME must be set for this test");
        // Exact HOME path (no trailing slash) should become just "~"
        assert_eq!(tilde_path(&home), "~");
    }

    #[test]
    fn test_run_removes_multiple_worktrees_with_merged_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create and merge two branches, each with a worktree
        for name in &["feature/wt-a", "feature/wt-b"] {
            StdCommand::new("git")
                .args(["checkout", "-b", name])
                .current_dir(path)
                .output()?;
            std::fs::write(path.join(format!("{}.txt", name.replace('/', "-"))), name)?;
            StdCommand::new("git")
                .args(["add", "."])
                .current_dir(path)
                .output()?;
            StdCommand::new("git")
                .args(["commit", "-m", &format!("{name} feature")])
                .current_dir(path)
                .output()?;
            StdCommand::new("git")
                .args(["checkout", "main"])
                .current_dir(path)
                .output()?;
            StdCommand::new("git")
                .args(["merge", name])
                .current_dir(path)
                .output()?;
        }

        let wt_a = path.join("wt-a");
        StdCommand::new("git")
            .args(["worktree", "add", wt_a.to_str().unwrap(), "feature/wt-a"])
            .current_dir(path)
            .output()?;

        let wt_b = path.join("wt-b");
        StdCommand::new("git")
            .args(["worktree", "add", wt_b.to_str().unwrap(), "feature/wt-b"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        // Both worktrees and branches should exist before cleanup
        assert!(wt_a.exists());
        assert!(wt_b.exists());
        let branches_before = git.local_branches()?;
        assert!(branches_before.contains(&"feature/wt-a".to_string()));
        assert!(branches_before.contains(&"feature/wt-b".to_string()));

        run(&git, &config, &ui, &opts)?;

        // Both worktrees should be removed along with the branches
        assert!(!wt_a.exists(), "worktree A should be removed");
        assert!(!wt_b.exists(), "worktree B should be removed");
        let branches_after = git.local_branches()?;
        assert!(!branches_after.contains(&"feature/wt-a".to_string()));
        assert!(!branches_after.contains(&"feature/wt-b".to_string()));
        assert!(branches_after.contains(&"main".to_string()));
        Ok(())
    }

    #[test]
    fn test_run_dry_run_preserves_worktrees() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create and merge a branch with a worktree
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/wt-dry"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("dry.txt"), "dry run")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "dry run feature"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "feature/wt-dry"])
            .current_dir(path)
            .output()?;

        let wt_path = path.join("wt-dry");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                wt_path.to_str().unwrap(),
                "feature/wt-dry",
            ])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.dry_run = true;

        run(&git, &config, &ui, &opts)?;

        // Dry run should preserve both worktree and branch
        assert!(wt_path.exists(), "worktree should survive dry run");
        let branches = git.local_branches()?;
        assert!(
            branches.contains(&"feature/wt-dry".to_string()),
            "branch should survive dry run"
        );
        Ok(())
    }

    #[test]
    fn test_run_unified_cleanup_branches_and_orphan_worktrees() -> Result<()> {
        // This test verifies that merged branches (with worktrees) and orphan
        // worktrees are all cleaned up in a single unified pass (no separate
        // orphan worktree phase).
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a merged branch with a worktree
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/with-wt"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("with-wt.txt"), "branch with worktree")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "feature with worktree"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "feature/with-wt"])
            .current_dir(path)
            .output()?;

        let branch_wt_path = path.join("wt-branch");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                branch_wt_path.to_str().unwrap(),
                "feature/with-wt",
            ])
            .current_dir(path)
            .output()?;

        // Create a branch with a worktree, then orphan it by deleting the branch ref
        StdCommand::new("git")
            .args(["branch", "feature/orphan-wt"])
            .current_dir(path)
            .output()?;

        let orphan_wt_path = path.join("wt-orphan");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                orphan_wt_path.to_str().unwrap(),
                "feature/orphan-wt",
            ])
            .current_dir(path)
            .output()?;

        // Delete the branch ref to make the worktree orphaned
        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/orphan-wt"])
            .current_dir(path)
            .output()?;

        // Verify setup
        assert!(branch_wt_path.exists(), "branch worktree should exist");
        assert!(orphan_wt_path.exists(), "orphan worktree should exist");
        let branches_before = Git::with_workdir(false, path).local_branches()?;
        assert!(branches_before.contains(&"feature/with-wt".to_string()));

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let opts = opts_yes_skip_network();

        run(&git, &config, &ui, &opts)?;

        // The merged branch and its worktree should be removed
        assert!(
            !branch_wt_path.exists(),
            "branch worktree should be removed"
        );
        let branches_after = git.local_branches()?;
        assert!(
            !branches_after.contains(&"feature/with-wt".to_string()),
            "merged branch should be deleted"
        );

        // The orphan worktree should also be removed (unified with the branch pass)
        assert!(
            !orphan_wt_path.exists(),
            "orphan worktree should be removed in the unified pass"
        );

        Ok(())
    }

    #[test]
    fn test_run_no_worktrees_skips_orphan_cleanup() -> Result<()> {
        // When --no-worktrees is set, orphan worktrees should not be touched
        // even though they share the same phase as branch deletion.
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a branch with a worktree, then orphan it
        StdCommand::new("git")
            .args(["branch", "feature/orphan-skip"])
            .current_dir(path)
            .output()?;

        let orphan_wt_path = path.join("wt-orphan-skip");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                orphan_wt_path.to_str().unwrap(),
                "feature/orphan-skip",
            ])
            .current_dir(path)
            .output()?;

        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/orphan-skip"])
            .current_dir(path)
            .output()?;

        assert!(orphan_wt_path.exists(), "orphan worktree should exist");

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.no_worktrees = true;

        run(&git, &config, &ui, &opts)?;

        // Orphan worktree should survive because --no-worktrees was set
        assert!(
            orphan_wt_path.exists(),
            "orphan worktree should survive with --no-worktrees"
        );
        Ok(())
    }

    #[test]
    fn test_run_dry_run_preserves_orphan_worktrees() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a branch with a worktree, then orphan it
        StdCommand::new("git")
            .args(["branch", "feature/orphan-dry"])
            .current_dir(path)
            .output()?;

        let orphan_wt_path = path.join("wt-orphan-dry");
        StdCommand::new("git")
            .args([
                "worktree",
                "add",
                orphan_wt_path.to_str().unwrap(),
                "feature/orphan-dry",
            ])
            .current_dir(path)
            .output()?;

        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/orphan-dry"])
            .current_dir(path)
            .output()?;

        assert!(orphan_wt_path.exists());

        let git = Git::with_workdir(false, path);
        let config = default_config();
        let ui = Ui::new();
        let mut opts = opts_yes_skip_network();
        opts.dry_run = true;

        run(&git, &config, &ui, &opts)?;

        // Dry run should preserve the orphan worktree
        assert!(
            orphan_wt_path.exists(),
            "orphan worktree should survive dry run"
        );
        Ok(())
    }
}
