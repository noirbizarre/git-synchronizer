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
fn resolve_merge_targets(git: &Git, config: &Config) -> Result<Vec<String>> {
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

    candidates.sort();
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
