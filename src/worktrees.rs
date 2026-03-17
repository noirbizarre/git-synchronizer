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

    fn init_repo_with_worktree() -> (tempfile::TempDir, Git, String) {
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

        // Create a branch and a worktree for it
        StdCommand::new("git")
            .args(["branch", "feature/wt"])
            .current_dir(path)
            .output()
            .unwrap();

        let wt_path = dir.path().join("worktree-feature");
        StdCommand::new("git")
            .args(["worktree", "add", wt_path.to_str().unwrap(), "feature/wt"])
            .current_dir(path)
            .output()
            .unwrap();

        let git = Git::with_workdir(false, path);
        (dir, git, wt_path.to_string_lossy().to_string())
    }

    #[test]
    fn test_find_worktrees_for_branches() {
        let (_dir, git, _wt_path) = init_repo_with_worktree();

        let worktrees = find_worktrees_for_branches(&git, &["feature/wt".to_string()]).unwrap();
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch.as_deref(), Some("feature/wt"));
    }

    #[test]
    fn test_find_worktrees_for_branches_no_match() {
        let (_dir, git, _wt_path) = init_repo_with_worktree();

        let worktrees = find_worktrees_for_branches(&git, &["nonexistent".to_string()]).unwrap();
        assert!(worktrees.is_empty());
    }

    #[test]
    fn test_find_orphan_worktrees_none_initially() {
        let (_dir, git, _wt_path) = init_repo_with_worktree();

        // All worktrees have existing branches, so no orphans
        let orphans = find_orphan_worktrees(&git).unwrap();
        assert!(orphans.is_empty());
    }

    #[test]
    fn test_find_orphan_worktrees_detects_orphan() {
        let (_dir, git, _wt_path) = init_repo_with_worktree();
        let path = _dir.path();

        // Use update-ref to delete the branch ref directly, bypassing the
        // check that prevents deleting a branch checked out in a worktree.
        StdCommand::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/wt"])
            .current_dir(path)
            .output()
            .unwrap();

        // Now the worktree's branch no longer exists, so it's orphaned
        let orphans = find_orphan_worktrees(&git).unwrap();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].branch.as_deref(), Some("feature/wt"));
    }
}
