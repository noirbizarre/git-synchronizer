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

/// Resolve protected patterns to actual existing local branch names.
///
/// Literal patterns (e.g. "main") are kept as-is if they exist.
/// Glob patterns (e.g. "release/*") are expanded to matching branches.
fn resolve_merge_targets(git: &Git, config: &Config) -> Result<Vec<String>> {
    let matcher = build_protected_matcher(config)?;
    let all_branches = git.local_branches()?;

    let targets: Vec<String> = all_branches
        .into_iter()
        .filter(|b| matcher.is_match(b))
        .collect();

    Ok(targets)
}

/// Return local branches that are merged into *any* of the protected branches
/// and are not themselves protected.
pub fn find_merged_local(git: &Git, config: &Config) -> Result<Vec<String>> {
    let matcher = build_protected_matcher(config)?;
    let current = git.current_branch()?;
    let targets = resolve_merge_targets(git, config)?;

    let mut candidates: Vec<String> = Vec::new();

    for target in &targets {
        let merged = git.merged_branches(target)?;
        for branch in merged {
            if branch == current {
                continue;
            }
            if matcher.is_match(&branch) {
                continue;
            }
            if !candidates.contains(&branch) {
                candidates.push(branch);
            }
        }
    }

    // Also check branches not caught by --merged (rebase merge detection via git cherry)
    let all_branches = git.local_branches()?;
    for branch in &all_branches {
        if candidates.contains(branch) || *branch == current || matcher.is_match(branch) {
            continue;
        }
        for target in &targets {
            if git.cherry_merged(target, branch).unwrap_or(false) && !candidates.contains(branch) {
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
    let targets = resolve_merge_targets(git, config)?;

    let mut candidates: Vec<String> = Vec::new();

    for target in &targets {
        let merged = git.merged_remote_branches(target, remote)?;
        for branch in merged {
            if matcher.is_match(&branch) {
                continue;
            }
            if !candidates.contains(&branch) {
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

        // Create a release branch (should be protected)
        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["checkout", "-b", "release/1.0"])
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

    #[test]
    fn test_build_protected_matcher() {
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            remotes: None,
        };
        let matcher = build_protected_matcher(&config).unwrap();
        assert!(matcher.is_match("main"));
        assert!(matcher.is_match("release/1.0"));
        assert!(matcher.is_match("release/2.0-beta"));
        assert!(!matcher.is_match("feature/foo"));
        assert!(!matcher.is_match("develop"));
    }

    #[test]
    fn test_find_merged_local() {
        let (_dir, git) = init_repo_with_branches();
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            remotes: None,
        };

        let merged = find_merged_local(&git, &config).unwrap();

        // feature/done was merged, so it should appear
        assert!(merged.contains(&"feature/done".to_string()));
        // feature/wip was NOT merged
        assert!(!merged.contains(&"feature/wip".to_string()));
        // main is protected
        assert!(!merged.contains(&"main".to_string()));
        // release/1.0 matches the release/* pattern
        assert!(!merged.contains(&"release/1.0".to_string()));
    }

    #[test]
    fn test_find_merged_local_excludes_current_branch() {
        let (_dir, git) = init_repo_with_branches();
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
        };

        let current = git.current_branch().unwrap();
        let merged = find_merged_local(&git, &config).unwrap();
        assert!(!merged.contains(&current));
    }

    #[test]
    fn test_find_merged_local_detects_cherry_picked_branches() {
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

        // Create a feature branch with a commit
        StdCommand::new("git")
            .args(["checkout", "-b", "feature/cherry"])
            .current_dir(path)
            .output()
            .unwrap();
        std::fs::write(path.join("cherry.txt"), "cherry").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "cherry feature"])
            .current_dir(path)
            .output()
            .unwrap();

        // Cherry-pick onto main (simulating a rebase merge)
        let log_output = StdCommand::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()
            .unwrap();
        let commit_sha = String::from_utf8_lossy(&log_output.stdout)
            .trim()
            .to_string();

        StdCommand::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()
            .unwrap();

        // Add a diverging commit on main so cherry-pick creates a distinct commit
        std::fs::write(path.join("diverge.txt"), "diverge").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();
        StdCommand::new("git")
            .args(["commit", "-m", "diverge"])
            .current_dir(path)
            .output()
            .unwrap();

        StdCommand::new("git")
            .args(["cherry-pick", &commit_sha])
            .current_dir(path)
            .output()
            .unwrap();

        let git = Git::with_workdir(false, path);

        // Cherry-picked branch should always be detected as merged
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
        };
        let merged = find_merged_local(&git, &config).unwrap();
        assert!(merged.contains(&"feature/cherry".to_string()));
    }

    #[test]
    fn test_resolve_merge_targets_with_globs() {
        let (_dir, git) = init_repo_with_branches();
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            remotes: None,
        };

        let targets = resolve_merge_targets(&git, &config).unwrap();
        assert!(targets.contains(&"main".to_string()));
        assert!(targets.contains(&"release/1.0".to_string()));
        assert!(!targets.contains(&"feature/done".to_string()));
        assert!(!targets.contains(&"feature/wip".to_string()));
    }

    #[test]
    fn test_find_merged_local_no_targets() {
        let (_dir, git) = init_repo_with_branches();
        // Use a pattern that matches nothing
        let config = Config {
            protected: vec!["nonexistent-branch".to_string()],
            remotes: None,
        };

        let merged = find_merged_local(&git, &config).unwrap();
        assert!(merged.is_empty());
    }
}
