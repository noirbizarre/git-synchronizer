use std::collections::HashSet;

use anyhow::Result;
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::Config;
use crate::git::Git;

/// Build a `GlobSet` from the protected branch patterns in config.
pub fn build_protected_matcher(config: &Config) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in &config.protected {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}

/// Check whether a branch is protected, considering both global glob patterns
/// and per-branch `branch.<name>.sync-protected` config.
fn is_protected(branch: &str, matcher: &GlobSet, branch_protected: &HashSet<String>) -> bool {
    matcher.is_match(branch) || branch_protected.contains(branch)
}

/// Resolve protected patterns to actual existing local branch names.
///
/// Literal patterns (e.g. "main") are kept as-is if they exist.
/// Glob patterns (e.g. "release/*") are expanded to matching branches.
/// Branches marked with per-branch `sync-protected` config are also included.
pub fn resolve_merge_targets(git: &Git, config: &Config) -> Result<Vec<String>> {
    let matcher = build_protected_matcher(config)?;
    let branch_protected: HashSet<String> = git.branch_protected_list()?.into_iter().collect();
    let all_branches = git.local_branches()?;

    let targets: Vec<String> = all_branches
        .into_iter()
        .filter(|b| is_protected(b, &matcher, &branch_protected))
        .collect();

    Ok(targets)
}

/// Return local branches that are merged into *any* of the protected branches
/// and are not themselves protected.
pub fn find_merged_local(git: &Git, config: &Config) -> Result<Vec<String>> {
    let matcher = build_protected_matcher(config)?;
    let branch_protected: HashSet<String> = git.branch_protected_list()?.into_iter().collect();
    let current = git.current_branch()?;
    let targets = resolve_merge_targets(git, config)?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut candidates: Vec<String> = Vec::new();

    for target in &targets {
        let merged = git.merged_branches(target)?;
        for branch in merged {
            if branch == current {
                continue;
            }
            if is_protected(&branch, &matcher, &branch_protected) {
                continue;
            }
            if seen.insert(branch.clone()) {
                candidates.push(branch);
            }
        }
    }

    // Also check branches not caught by --merged (rebase merge detection via git cherry)
    let all_branches = git.local_branches()?;
    for branch in &all_branches {
        if seen.contains(branch)
            || *branch == current
            || is_protected(branch, &matcher, &branch_protected)
        {
            continue;
        }
        for target in &targets {
            if git.cherry_merged(target, branch).unwrap_or(false) && seen.insert(branch.clone()) {
                candidates.push(branch.clone());
                break;
            }
        }
    }

    // Fast tree-SHA comparison: detects branches whose tree object is
    // identical to a target's tree (cheapest content-equality check).
    for branch in &all_branches {
        if seen.contains(branch)
            || *branch == current
            || is_protected(branch, &matcher, &branch_protected)
        {
            continue;
        }
        for target in &targets {
            if git.trees_match(target, branch).unwrap_or(false) && seen.insert(branch.clone()) {
                candidates.push(branch.clone());
                break;
            }
        }
    }

    // Also check branches via empty diff (catches squash-merge cases
    // where the target tree already contains all branch changes)
    for branch in &all_branches {
        if seen.contains(branch)
            || *branch == current
            || is_protected(branch, &matcher, &branch_protected)
        {
            continue;
        }
        for target in &targets {
            if git.diff_empty(target, branch).unwrap_or(false) && seen.insert(branch.clone()) {
                candidates.push(branch.clone());
                break;
            }
        }
    }

    // Patch-ID comparison: catches branches whose commits were re-applied
    // on target with different SHAs (rebase + reword, partial cherry-pick,
    // history rewrite). Fallback when cherry / tree / diff strategies miss.
    for branch in &all_branches {
        if seen.contains(branch)
            || *branch == current
            || is_protected(branch, &matcher, &branch_protected)
        {
            continue;
        }
        for target in &targets {
            if git.patch_id_match(target, branch).unwrap_or(false) && seen.insert(branch.clone()) {
                candidates.push(branch.clone());
                break;
            }
        }
    }

    // Simulated merge: in-memory merge yields target's existing tree
    // (catches squash-merged branches where target has advanced with
    // unrelated commits touching different files — defeats both
    // `trees_match` and `diff_empty`). Placed last because it is the most
    // expensive check.
    for branch in &all_branches {
        if seen.contains(branch)
            || *branch == current
            || is_protected(branch, &matcher, &branch_protected)
        {
            continue;
        }
        for target in &targets {
            if git.merge_adds_nothing(target, branch).unwrap_or(false)
                && seen.insert(branch.clone())
            {
                candidates.push(branch.clone());
                break;
            }
        }
    }

    // Squash patch-id: combined patch-id of the branch's full diff matches
    // the patch-id of a single squash commit on target. Catches squash-
    // merges that defeat both per-commit `patch_id_match` (N branch commits
    // collapsed into 1 squash) and textual `merge_adds_nothing` (squash
    // commit on target was later edited so the in-memory merge re-applies
    // branch text textually). `git patch-id` ignores whitespace and context
    // lines, so it survives review-time formatting tweaks.
    for branch in &all_branches {
        if seen.contains(branch)
            || *branch == current
            || is_protected(branch, &matcher, &branch_protected)
        {
            continue;
        }
        for target in &targets {
            if git.squash_patch_id_match(target, branch).unwrap_or(false)
                && seen.insert(branch.clone())
            {
                candidates.push(branch.clone());
                break;
            }
        }
    }

    candidates.sort();
    Ok(candidates)
}

/// Return local branches whose upstream tracking branch no longer exists.
///
/// A deleted upstream is the footprint left by a merged pull request whose
/// remote branch was removed. It is the only reliable signal for branches that
/// were squash-merged into a target which has since advanced far enough for the
/// content-based strategies in [`find_merged_local`] to lose the trail.
///
/// Because the signal is weaker (an upstream can also vanish because someone
/// deleted an *unmerged* remote branch), these branches are reported as their
/// own category and are not pre-selected for deletion. Callers must also ensure
/// remote-tracking refs were refreshed with `git fetch --prune` beforehand,
/// otherwise the result is meaningless.
///
/// Protected branches, the current branch, and anything already reported by
/// [`find_merged_local`] (passed as `merged`) are excluded.
pub fn find_gone_local(git: &Git, config: &Config, merged: &[String]) -> Result<Vec<String>> {
    let matcher = build_protected_matcher(config)?;
    let branch_protected: HashSet<String> = git.branch_protected_list()?.into_iter().collect();
    let current = git.current_branch()?;
    let already: HashSet<&String> = merged.iter().collect();

    let mut candidates: Vec<String> = git
        .branches_with_gone_upstream()?
        .into_iter()
        .filter(|b| {
            *b != current && !already.contains(b) && !is_protected(b, &matcher, &branch_protected)
        })
        .collect();

    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

/// Return remote branches that are merged into *any* of the protected branches
/// and are not themselves protected, for the given remote.
pub fn find_merged_remote(git: &Git, config: &Config, remote: &str) -> Result<Vec<String>> {
    let matcher = build_protected_matcher(config)?;
    let branch_protected: HashSet<String> = git.branch_protected_list()?.into_iter().collect();
    let targets = resolve_merge_targets(git, config)?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut candidates: Vec<String> = Vec::new();

    for target in &targets {
        let merged = git.merged_remote_branches(target, remote)?;
        for branch in merged {
            if is_protected(&branch, &matcher, &branch_protected) {
                continue;
            }
            if seen.insert(branch.clone()) {
                candidates.push(branch);
            }
        }
    }

    candidates.sort();
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    /// Create a repo with branches plus an additional `release/1.0` branch.
    fn init_repo_with_release() -> Result<(tempfile::TempDir, Git)> {
        let (dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let path = dir.path();
        StdCommand::new("git")
            .args(["checkout", "-b", "release/1.0"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Ok((dir, git))
    }

    #[test]
    fn test_build_protected_matcher() -> Result<()> {
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let matcher = build_protected_matcher(&config)?;
        assert!(matcher.is_match("main"));
        assert!(matcher.is_match("release/1.0"));
        assert!(matcher.is_match("release/2.0-beta"));
        assert!(!matcher.is_match("feature/foo"));
        assert!(!matcher.is_match("develop"));
        Ok(())
    }

    #[test]
    fn test_find_merged_local() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            remotes: None,
            worktrunk: None,
        };

        let merged = find_merged_local(&git, &config)?;

        // feature/done was merged, so it should appear
        assert!(merged.contains(&"feature/done".to_string()));
        // feature/wip was NOT merged
        assert!(!merged.contains(&"feature/wip".to_string()));
        // main is protected
        assert!(!merged.contains(&"main".to_string()));
        // release/1.0 matches the release/* pattern
        assert!(!merged.contains(&"release/1.0".to_string()));
        Ok(())
    }

    #[test]
    fn test_find_gone_local() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_gone_upstream()?;
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };

        let merged = find_merged_local(&git, &config)?;
        let gone = find_gone_local(&git, &config, &merged)?;

        // Upstream was deleted and pruned.
        assert_eq!(gone, vec!["feature/gone".to_string()]);
        // Upstream still exists.
        assert!(!gone.contains(&"feature/alive".to_string()));
        // No upstream at all carries no signal.
        assert!(!gone.contains(&"feature/done".to_string()));
        // Protected target is never a candidate.
        assert!(!gone.contains(&"main".to_string()));
        Ok(())
    }

    #[test]
    fn test_find_gone_local_excludes_already_merged() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_gone_upstream()?;
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };

        // Pretend the content-based strategies already caught it.
        let merged = vec!["feature/gone".to_string()];
        assert!(find_gone_local(&git, &config, &merged)?.is_empty());
        Ok(())
    }

    #[test]
    fn test_find_gone_local_excludes_current_and_protected() -> Result<()> {
        let (dir, git) = crate::test_helpers::init_repo_with_gone_upstream()?;

        // Protected by pattern.
        let config = Config {
            protected: vec!["main".to_string(), "feature/*".to_string()],
            remotes: None,
            worktrunk: None,
        };
        assert!(find_gone_local(&git, &config, &[])?.is_empty());

        // Checked out: never proposed for deletion.
        StdCommand::new("git")
            .args(["checkout", "feature/gone"])
            .current_dir(dir.path().join("work"))
            .output()?;
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        assert!(find_gone_local(&git, &config, &[])?.is_empty());
        Ok(())
    }

    #[test]
    fn test_find_merged_local_excludes_current_branch() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };

        let current = git.current_branch()?;
        let merged = find_merged_local(&git, &config)?;
        assert!(!merged.contains(&current));
        Ok(())
    }

    #[test]
    fn test_find_merged_local_detects_cherry_picked_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch with a commit
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/cherry"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("cherry.txt"), "cherry")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "cherry feature"])
            .current_dir(path)
            .output()?;

        // Cherry-pick onto main (simulating a rebase merge)
        let log_output = StdCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()?;
        let commit_sha = String::from_utf8_lossy(&log_output.stdout)
            .trim()
            .to_string();

        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;

        // Add a diverging commit on main so cherry-pick creates a distinct commit
        std::fs::write(path.join("diverge.txt"), "diverge")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "diverge"])
            .current_dir(path)
            .output()?;

        StdCommand::new("git")
            .args(["cherry-pick", &commit_sha])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Cherry-picked branch should always be detected as merged
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let merged = find_merged_local(&git, &config)?;
        assert!(merged.contains(&"feature/cherry".to_string()));
        Ok(())
    }

    #[test]
    fn test_find_merged_local_detects_squash_merged_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch with a commit
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/squash"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("squash.txt"), "squash")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "squash feature"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main (creates a single squash commit, not a merge commit)
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "--squash", "feature/squash"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "squash merge"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Squash-merged branch should be detected via empty three-dot diff
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let merged = find_merged_local(&git, &config)?;
        assert!(
            merged.contains(&"feature/squash".to_string()),
            "squash-merged branch should be detected as merged"
        );
        Ok(())
    }

    #[test]
    fn test_find_merged_local_detects_tree_match_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch with a commit
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/tree-match"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("tree.txt"), "tree content")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "tree feature"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main so both tips share the same tree object
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "--squash", "feature/tree-match"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "squash merge tree"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Branch should be detected via tree SHA comparison
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let merged = find_merged_local(&git, &config)?;
        assert!(
            merged.contains(&"feature/tree-match".to_string()),
            "branch with matching tree SHA should be detected as merged"
        );
        Ok(())
    }

    #[test]
    fn test_find_merged_local_detects_patch_id_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Feature branch with one commit.
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/patch-id"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("patch.txt"), "patch content\n")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "patch-id feature"])
            .current_dir(path)
            .output()?;
        let sha = String::from_utf8_lossy(
            &StdCommand::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(path)
                .output()?
                .stdout,
        )
        .trim()
        .to_string();

        // Diverge main, then cherry-pick + amend so the SHA changes but
        // the diff (and therefore the patch-id) is preserved. `git cherry`
        // should miss it (different author/committer date after amend may
        // still be matched, so we also tweak the message).
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("diverge.txt"), "diverge\n")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "diverge"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["cherry-pick", &sha])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "--amend", "-m", "patch-id feature (reworded)"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let merged = find_merged_local(&git, &config)?;
        assert!(
            merged.contains(&"feature/patch-id".to_string()),
            "branch with matching patch-id should be detected as merged"
        );
        Ok(())
    }

    #[test]
    fn test_find_merged_local_detects_simulated_merge_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch that touches a.txt
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/squash-advanced"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("a.txt"), "feature content")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "feature: a.txt"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "--squash", "feature/squash-advanced"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "squash merge"])
            .current_dir(path)
            .output()?;

        // Advance main with an unrelated commit touching a different file,
        // so neither trees_match nor diff_empty would detect the branch.
        std::fs::write(path.join("b.txt"), "unrelated")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "main: unrelated b.txt"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let merged = find_merged_local(&git, &config)?;
        assert!(
            merged.contains(&"feature/squash-advanced".to_string()),
            "squash-merged branch with advanced target should be detected via simulated merge"
        );
        Ok(())
    }

    #[test]
    fn test_find_merged_local_detects_multi_commit_squash_via_patch_id() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Feature branch with TWO commits — combined diff is a single
        // addition to a.txt. This is the canonical case `patch_id_match`
        // (per-commit) cannot resolve and where `merge_adds_nothing` may
        // also miss when the squashed commit on target differs textually
        // from the branch's commits.
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/multi-commit-squash"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("a.txt"), "line1\n")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "feature: line1"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("a.txt"), "line1\nline2\n")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "feature: line2"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main as a single commit.
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["merge", "--squash", "feature/multi-commit-squash"])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "squash merge"])
            .current_dir(path)
            .output()?;

        // Unrelated advance on main (defeats trees_match / diff_empty).
        std::fs::write(path.join("b.txt"), "unrelated")?;
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        StdCommand::new("git")
            .args(["commit", "-m", "main: unrelated b.txt"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        let merged = find_merged_local(&git, &config)?;
        assert!(
            merged.contains(&"feature/multi-commit-squash".to_string()),
            "multi-commit branch squash-merged into main should be detected \
             via squash patch-id"
        );
        Ok(())
    }

    #[test]
    fn test_resolve_merge_targets_with_globs() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            remotes: None,
            worktrunk: None,
        };

        let targets = resolve_merge_targets(&git, &config)?;
        assert!(targets.contains(&"main".to_string()));
        assert!(targets.contains(&"release/1.0".to_string()));
        assert!(!targets.contains(&"feature/done".to_string()));
        assert!(!targets.contains(&"feature/wip".to_string()));
        Ok(())
    }

    #[test]
    fn test_find_merged_local_no_targets() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        // Use a pattern that matches nothing
        let config = Config {
            protected: vec!["nonexistent-branch".to_string()],
            remotes: None,
            worktrunk: None,
        };

        let merged = find_merged_local(&git, &config)?;
        assert!(merged.is_empty());
        Ok(())
    }

    #[test]
    fn test_find_merged_local_respects_branch_protected() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };

        // Without per-branch protection, feature/done should be a candidate
        let merged = find_merged_local(&git, &config)?;
        assert!(merged.contains(&"feature/done".to_string()));

        // Mark feature/done as per-branch protected
        git.set_branch_protected("feature/done", true)?;
        let merged = find_merged_local(&git, &config)?;
        assert!(
            !merged.contains(&"feature/done".to_string()),
            "per-branch protected branch should not be a deletion candidate"
        );

        // Clean up
        git.set_branch_protected("feature/done", false)?;
        Ok(())
    }

    #[test]
    fn test_branch_protected_serves_as_merge_target() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        // Only use per-branch protection on "main" (no global patterns match anything)
        let config = Config {
            protected: vec!["nonexistent-branch".to_string()],
            remotes: None,
            worktrunk: None,
        };

        // Without any real protected branches, nothing is a merge target
        let merged = find_merged_local(&git, &config)?;
        assert!(merged.is_empty());

        // Mark "main" as per-branch protected — it should now be a merge target
        git.set_branch_protected("main", true)?;
        let merged = find_merged_local(&git, &config)?;
        assert!(
            merged.contains(&"feature/done".to_string()),
            "branches merged into a per-branch protected branch should be candidates"
        );

        // Clean up
        git.set_branch_protected("main", false)?;
        Ok(())
    }
}
