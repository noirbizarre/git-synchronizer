use anyhow::Result;

use crate::git::{Git, Worktree};

/// Find worktrees whose branch no longer exists locally.
pub fn find_orphan_worktrees(git: &Git) -> Result<Vec<Worktree>> {
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
                Some(branch) => !local_branches.contains(branch),
                // Detached HEAD worktrees are not considered orphans
                None => false,
            }
        })
        .collect();

    Ok(orphans)
}

/// Find worktrees whose branch is in the list of branches about to be deleted.
#[cfg(test)]
pub fn find_worktrees_for_branches(git: &Git, branches: &[String]) -> Result<Vec<Worktree>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    #[test]
    fn test_find_worktrees_for_branches() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;

        let worktrees = find_worktrees_for_branches(&git, &["feature/wt".to_string()])?;
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch.as_deref(), Some("feature/wt"));
        Ok(())
    }

    #[test]
    fn test_find_worktrees_for_branches_no_match() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;

        let worktrees = find_worktrees_for_branches(&git, &["nonexistent".to_string()])?;
        assert!(worktrees.is_empty());
        Ok(())
    }

    #[test]
    fn test_find_orphan_worktrees_none_initially() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;

        // All worktrees have existing branches, so no orphans
        let orphans = find_orphan_worktrees(&git)?;
        assert!(orphans.is_empty());
        Ok(())
    }

    #[test]
    fn test_find_orphan_worktrees_detects_orphan() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let path = _dir.path();

        // Use update-ref to delete the branch ref directly, bypassing the
        // check that prevents deleting a branch checked out in a worktree.
        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/wt"])
            .current_dir(path)
            .output()?;

        // Now the worktree's branch no longer exists, so it's orphaned
        let orphans = find_orphan_worktrees(&git)?;
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].branch.as_deref(), Some("feature/wt"));
        Ok(())
    }
}
