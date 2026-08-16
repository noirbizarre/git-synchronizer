//! Discovery of linked worktrees, including orphans.
//!
//! An orphan is a worktree whose branch no longer exists locally — the residue
//! of a branch deleted without removing its worktree first.

use std::fs;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::branches::Filter;
use crate::duration::MinAge;
use crate::git::{Git, Worktree};

/// Find worktrees whose branch no longer exists locally.
///
/// Worktrees holding an ignored branch are skipped: git-wipe must not touch
/// them even if their branch ref has disappeared.
pub fn find_orphan_worktrees(git: &Git, filter: &Filter) -> Result<Vec<Worktree>> {
    let worktrees = git.worktree_list()?;
    let local_branches = git.local_branches()?;

    let orphans: Vec<Worktree> = worktrees
        .into_iter()
        .filter(|wt| {
            // Skip the main worktree (bare) and worktrees without a branch
            if wt.is_bare {
                return false;
            }
            match &wt.branch {
                Some(branch) => !filter.is_ignored(branch) && !local_branches.contains(branch),
                // Detached HEAD worktrees are not considered orphans
                None => false,
            }
        })
        .collect();

    Ok(orphans)
}

/// Find worktrees whose branch is in the list of branches about to be deleted.
#[cfg(test)]
fn find_branch_worktrees(git: &Git, branches: &[String]) -> Result<Vec<Worktree>> {
    let worktrees = git.worktree_list()?;

    let matching: Vec<Worktree> = worktrees
        .into_iter()
        .filter(|wt| {
            if wt.is_bare {
                return false;
            }
            match &wt.branch {
                Some(branch) => branches.contains(branch),
                None => false,
            }
        })
        .collect();

    Ok(matching)
}

/// How long ago a worktree was created.
///
/// Measured from the birth time of the worktree's administrative directory,
/// which `git worktree add` creates once and never recreates. The checkout
/// directory itself is not used: build output at its root perturbs its
/// timestamps. The branch tip date is not used either, since a worktree
/// created from an old default branch would then look ancient — the very case
/// `--min-age` exists to protect.
///
/// Returns `None` when the age cannot be established (unreadable metadata, a
/// filesystem without birth *or* modification times, or a clock that moved
/// backwards). Callers must treat that as "old enough": the guard is a safety
/// net, not a reason to refuse work.
pub fn worktree_age(git: &Git, wt: &Worktree) -> Option<Duration> {
    let admin_dir = git.worktree_git_dir(&wt.path).ok()?;
    let metadata = fs::metadata(&admin_dir).ok()?;
    // Birth time is the right answer but is not available on every platform or
    // filesystem; the admin dir's mtime is the closest available stand-in.
    let created = metadata.created().or_else(|_| metadata.modified()).ok()?;
    SystemTime::now().duration_since(created).ok()
}

/// Whether `wt` was created less than `min_age` ago and must be left alone.
///
/// A zero `min_age` short-circuits, so the default configuration costs no
/// extra `git` invocation.
pub fn is_too_young(git: &Git, wt: &Worktree, min_age: MinAge) -> bool {
    if min_age.is_zero() {
        return false;
    }
    worktree_age(git, wt).is_some_and(|age| age < min_age.as_duration())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::process::Command;

    #[test]
    fn find_branch_worktrees_matches_linked_worktrees() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;

        let worktrees = find_branch_worktrees(&git, &["feature/wt".to_string()])?;
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch.as_deref(), Some("feature/wt"));
        Ok(())
    }

    #[test]
    fn find_branch_worktrees_returns_nothing_without_a_match() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;

        let worktrees = find_branch_worktrees(&git, &["nonexistent".to_string()])?;
        assert!(worktrees.is_empty());
        Ok(())
    }

    #[test]
    fn find_orphan_worktrees_none_initially() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;

        // All worktrees have existing branches, so no orphans
        let orphans = find_orphan_worktrees(&git, &Filter::load(&git, &Config::default())?)?;
        assert!(orphans.is_empty());
        Ok(())
    }

    #[test]
    fn find_orphan_worktrees_detects_orphan() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let path = _dir.path();

        // Use update-ref to delete the branch ref directly, bypassing the
        // check that prevents deleting a branch checked out in a worktree.
        Command::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/wt"])
            .current_dir(path)
            .output()?;

        // Now the worktree's branch no longer exists, so it's orphaned
        let orphans = find_orphan_worktrees(&git, &Filter::load(&git, &Config::default())?)?;
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].branch.as_deref(), Some("feature/wt"));
        Ok(())
    }

    #[test]
    fn worktree_age_of_a_fresh_worktree_is_small() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let wt = worktree(&git)?;

        let age = worktree_age(&git, &wt).expect("age should be resolvable");
        assert!(age < Duration::from_secs(60), "unexpected age: {age:?}");
        Ok(())
    }

    #[test]
    fn fresh_worktree_is_too_young() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let wt = worktree(&git)?;

        assert!(is_too_young(&git, &wt, "1h".parse()?));
        Ok(())
    }

    #[test]
    fn zero_min_age_never_reports_a_worktree_as_young() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let wt = worktree(&git)?;

        assert!(!is_too_young(&git, &wt, MinAge::default()));
        Ok(())
    }

    /// The single linked worktree of the `init_repo_with_worktree` fixture.
    fn worktree(git: &Git) -> Result<Worktree> {
        let worktrees = find_branch_worktrees(git, &["feature/wt".to_string()])?;
        Ok(worktrees.into_iter().next().expect("fixture worktree"))
    }
}
