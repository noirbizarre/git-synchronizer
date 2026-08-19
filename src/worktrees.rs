//! Discovery of linked worktrees, including orphans.
//!
//! An orphan is a worktree whose branch no longer exists locally — the residue
//! of a branch deleted without removing its worktree first.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

/// How long ago a worktree was last meaningfully changed.
///
/// Age is the most recent of:
/// - the newest mtime among tracked and untracked-but-not-ignored files
///   (`git ls-files --cached --others --exclude-standard`), and
/// - the committer date of `HEAD`.
///
/// Gitignored churn — `target/`, `node_modules/`, build output, caches — is
/// excluded from the mtime scan, so a build or a dependency install cannot
/// make a months-stale worktree look fresh. A freshly checked-out file's
/// mtime is set to checkout time, so a worktree just created from a stale
/// default branch still reads as fresh even though `HEAD`'s committer date
/// is old — the very case `--min-age` exists to protect.
///
/// Falls back to the birth (or modification) time of the worktree's
/// administrative directory — the previous metric — when the tree cannot be
/// enumerated: an orphan whose checkout was removed, a corrupted index, or
/// another broken worktree.
///
/// Returns `None` when the age cannot be established by either means
/// (unreadable metadata, a filesystem without birth *or* modification
/// times, or a clock that moved backwards). Callers must treat that as "old
/// enough": the guard is a safety net, not a reason to refuse work.
pub fn worktree_age(git: &Git, wt: &Worktree) -> Option<Duration> {
    let reference = last_real_change(git, &wt.path).or_else(|| admin_dir_time(git, wt));
    SystemTime::now().duration_since(reference?).ok()
}

/// The newest of the tracked/untracked file mtimes and `HEAD`'s committer
/// date, or `None` when the worktree's tree cannot be enumerated.
fn last_real_change(git: &Git, path: &Path) -> Option<SystemTime> {
    let files = git.worktree_files(path).ok()?;
    let newest_file = files
        .iter()
        .filter_map(|f| fs::metadata(path.join(f)).ok()?.modified().ok())
        .max();

    let head_date = git
        .committer_date(path, "HEAD")
        .ok()
        .flatten()
        .map(|secs| UNIX_EPOCH + Duration::from_secs(secs));

    match (newest_file, head_date) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

/// The previous metric: birth (or modification) time of the worktree's
/// administrative directory. Used only as a fallback now.
fn admin_dir_time(git: &Git, wt: &Worktree) -> Option<SystemTime> {
    let admin_dir = git.worktree_git_dir(&wt.path).ok()?;
    let metadata = fs::metadata(&admin_dir).ok()?;
    metadata.created().or_else(|_| metadata.modified()).ok()
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

    #[test]
    fn gitignored_churn_does_not_refresh_an_old_worktrees_age() -> Result<()> {
        let (_dir, git, wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let wt = worktree(&git)?;
        let wt_dir = std::path::Path::new(&wt_path);

        // Track a .gitignore so a later gitignored file is genuinely
        // excluded, then push both tracked files' mtimes and HEAD's
        // committer date into the past.
        std::fs::write(wt_dir.join(".gitignore"), "target/\n")?;
        Command::new("git")
            .args(["add", ".gitignore"])
            .current_dir(wt_dir)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "add gitignore"])
            .current_dir(wt_dir)
            .output()?;

        const TWO_DAYS: u64 = 2 * 24 * 3600;
        for name in ["README.md", ".gitignore"] {
            backdate_file(&wt_dir.join(name), TWO_DAYS)?;
        }
        backdate_head(wt_dir, TWO_DAYS)?;

        let baseline = worktree_age(&git, &wt).expect("age should resolve");
        assert!(
            baseline > Duration::from_secs(23 * 3600),
            "backdating did not take effect: {baseline:?}"
        );

        // Touch only a gitignored file.
        std::fs::create_dir_all(wt_dir.join("target"))?;
        std::fs::write(wt_dir.join("target/build.log"), "noise")?;

        let age = worktree_age(&git, &wt).expect("age should resolve");
        assert!(
            age > Duration::from_secs(23 * 3600),
            "gitignored churn made an old worktree look fresh: {age:?}"
        );
        Ok(())
    }

    #[test]
    fn an_ancient_head_does_not_make_a_freshly_checked_out_worktree_look_old() -> Result<()> {
        let (_dir, git, wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let wt = worktree(&git)?;
        let wt_dir = std::path::Path::new(&wt_path);

        // Files were just checked out, so their mtimes are fresh; only
        // backdate HEAD's committer date.
        backdate_head(wt_dir, 2 * 24 * 3600)?;

        let age = worktree_age(&git, &wt).expect("age should resolve");
        assert!(
            age < Duration::from_secs(60),
            "an ancient HEAD made a fresh worktree look old: {age:?}"
        );
        Ok(())
    }

    #[test]
    fn worktree_age_resolves_for_an_orphaned_worktree() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let path = _dir.path();

        // Delete the branch ref directly, bypassing the check that prevents
        // deleting a branch checked out in a worktree.
        Command::new("git")
            .args(["update-ref", "-d", "refs/heads/feature/wt"])
            .current_dir(path)
            .output()?;

        let orphans = find_orphan_worktrees(&git, &Filter::load(&git, &Config::default())?)?;
        let wt = orphans.into_iter().next().expect("orphan worktree");

        let age = worktree_age(&git, &wt);
        assert!(
            age.is_some(),
            "age should still resolve when the tree is intact but the branch is gone"
        );
        Ok(())
    }

    #[test]
    fn worktree_age_falls_back_to_the_admin_dir_when_the_index_is_unreadable() -> Result<()> {
        let (_dir, git, _wt_path) = crate::test_helpers::init_repo_with_worktree()?;
        let wt = worktree(&git)?;

        // Corrupt the worktree's own index so `git ls-files` fails, forcing
        // the fallback to the admin-dir metric.
        let index_path = git.worktree_git_dir(&wt.path)?.join("index");
        std::fs::write(&index_path, b"not-an-index")?;

        let age = worktree_age(&git, &wt);
        assert!(
            age.is_some_and(|age| age < Duration::from_secs(60)),
            "should fall back to the freshly created admin dir: {age:?}"
        );
        Ok(())
    }

    /// The single linked worktree of the `init_repo_with_worktree` fixture.
    fn worktree(git: &Git) -> Result<Worktree> {
        let worktrees = find_branch_worktrees(git, &["feature/wt".to_string()])?;
        Ok(worktrees.into_iter().next().expect("fixture worktree"))
    }

    /// Push a file's mtime `secs_ago` into the past.
    fn backdate_file(path: &std::path::Path, secs_ago: u64) -> Result<()> {
        let file = std::fs::File::options().write(true).open(path)?;
        file.set_modified(SystemTime::now() - Duration::from_secs(secs_ago))?;
        Ok(())
    }

    /// Amend the commit at `HEAD` in `repo` so its committer date is
    /// `secs_ago` in the past, without touching the working tree (the tree
    /// content is unchanged, so `git commit --amend` does not rewrite files).
    fn backdate_head(repo: &std::path::Path, secs_ago: u64) -> Result<()> {
        let epoch = (SystemTime::now() - Duration::from_secs(secs_ago))
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        Command::new("git")
            .args([
                "commit",
                "--amend",
                "--no-edit",
                "--date",
                &format!("@{epoch}"),
            ])
            .env("GIT_COMMITTER_DATE", format!("@{epoch}"))
            .current_dir(repo)
            .output()?;
        Ok(())
    }
}
