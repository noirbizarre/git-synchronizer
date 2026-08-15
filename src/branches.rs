//! Branch classification and merge detection.
//!
//! [`Filter`] decides which branches are protected or ignored;
//! [`find_merged_local`] and its siblings decide which are merged. Merge
//! detection runs several strategies from cheapest to most expensive, because
//! no single git command recognises rebases, squashes and plain merges alike.

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use anyhow::{Result, anyhow};
use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::config::Config;
use crate::git::Git;

/// Build a `GlobSet` from a list of glob patterns.
fn build_matcher(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}

/// Branch classification: which branches are off-limits and why.
///
/// Two independent mechanisms feed each category: global glob patterns from the
/// `[sync]` config section, and per-branch git config flags
/// (`branch.<name>.sync-protected` / `branch.<name>.sync-ignored`).
#[derive(Debug)]
pub struct Filter {
    /// Glob patterns from `sync.protected`.
    protected: GlobSet,
    /// Branches flagged with `branch.<name>.sync-protected`.
    protected_branches: HashSet<String>,
    /// Glob patterns from `sync.ignore`.
    ignored: GlobSet,
    /// Branches flagged with `branch.<name>.sync-ignored`.
    ignored_branches: HashSet<String>,
}

impl Filter {
    /// Read both glob patterns and per-branch flags from the repository.
    pub fn load(git: &Git, config: &Config) -> Result<Self> {
        Ok(Self {
            protected: build_matcher(&config.protected)?,
            protected_branches: git.branch_protected_list()?.into_iter().collect(),
            ignored: build_matcher(&config.ignore)?,
            ignored_branches: git.branch_ignored_list()?.into_iter().collect(),
        })
    }

    /// Whether git-sync should pretend the branch does not exist.
    pub fn is_ignored(&self, branch: &str) -> bool {
        self.ignored.is_match(branch) || self.ignored_branches.contains(branch)
    }

    /// Whether the branch is protected from deletion and usable as a merge
    /// target. Ignoring wins over protection: an ignored branch is invisible,
    /// so it never becomes a target either.
    pub fn is_protected(&self, branch: &str) -> bool {
        !self.is_ignored(branch)
            && (self.protected.is_match(branch) || self.protected_branches.contains(branch))
    }

    /// Whether the branch must be kept out of the deletion candidate list.
    pub fn is_excluded(&self, branch: &str) -> bool {
        self.is_ignored(branch) || self.is_protected(branch)
    }
}

/// Resolve protected patterns to actual existing local branch names.
///
/// Literal patterns (e.g. "main") are kept as-is if they exist.
/// Glob patterns (e.g. "release/*") are expanded to matching branches.
/// Branches marked with per-branch `sync-protected` config are also included.
/// Ignored branches are never returned.
pub fn resolve_merge_targets(git: &Git, filter: &Filter) -> Result<Vec<String>> {
    let all_branches = git.local_branches()?;

    let targets: Vec<String> = all_branches
        .into_iter()
        .filter(|b| filter.is_protected(b))
        .collect();

    Ok(targets)
}

/// How thorough merge detection should be, trading speed for accuracy.
///
/// Each level is cumulative: it runs everything the previous level runs, plus
/// its own strategies. The default is [`Effort::Standard`], which keeps the
/// scan cheap enough to run on every invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Effort {
    /// Ancestor merges only (`git branch --merged`). Fastest.
    Quick = 1,
    /// Adds the cheap content-equality strategies: `git cherry`, identical
    /// tree SHA and empty three-dot diff.
    #[default]
    Standard = 2,
    /// Adds the expensive strategies: patch-id matching, simulated merge and
    /// combined squash patch-id. Most thorough, noticeably slower.
    Thorough = 3,
}

impl Effort {
    /// The numeric level, as accepted by `--effort` and `sync.effort`.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Human-readable name, used in `git sync config list`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quick => "quick",
            Self::Standard => "standard",
            Self::Thorough => "thorough",
        }
    }
}

impl TryFrom<u8> for Effort {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Quick),
            2 => Ok(Self::Standard),
            3 => Ok(Self::Thorough),
            other => Err(anyhow!("invalid effort level {other}, expected 1, 2 or 3")),
        }
    }
}

impl FromStr for Effort {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let value: u8 = s
            .trim()
            .parse()
            .map_err(|_| anyhow!("invalid effort level {s:?}, expected 1, 2 or 3"))?;
        Self::try_from(value)
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.as_u8(), self.label())
    }
}

/// Serialized as its numeric level, matching `--effort` and `sync.effort`.
impl serde::Serialize for Effort {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.as_u8())
    }
}

/// Outcome of a merged-branch scan.
///
/// Merge detection deliberately survives the failure of an individual
/// strategy — `merge_adds_nothing` needs git >= 2.38, and a single unreadable
/// ref should not hide every other merged branch. Those failures are collected
/// here instead of being discarded, so the caller can surface them once the
/// scan (and its spinner) has finished.
#[derive(Debug, Default)]
pub struct Merged {
    /// Branches detected as merged into a protected target.
    pub candidates: Vec<String>,
    /// One message per distinct merge-detection failure.
    pub warnings: Vec<String>,
}

/// A branch to probe: the name reported back to the caller, and the ref handed
/// to git.
///
/// The two differ for remote-tracking branches, which are reported (and
/// deleted) as `feature/x` but must be compared as `origin/feature/x`.
type Candidate = (String, String);

/// Run the content-based detection strategies enabled by `effort` over
/// `pool`, appending anything newly detected to `out`.
///
/// `targets` are full refs handed straight to git (`main` locally,
/// `origin/main` for remotes). `seen` carries over the branches already
/// detected by the caller's `--merged` pass, so each branch is reported once
/// and skipped by the remaining, more expensive strategies.
fn run_strategies(
    git: &Git,
    targets: &[String],
    pool: &[Candidate],
    effort: Effort,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    // Each pass below is a distinct merge-detection strategy, ordered from
    // cheapest to most expensive. A branch caught by an earlier pass is skipped
    // by the later ones via `seen`.
    type Strategy = fn(&Git, &str, &str) -> Result<bool>;

    // The `git branch --merged` pass run by the callers is Effort::Quick on its
    // own. The strategies below are enabled progressively:
    //
    // Effort::Standard (default) adds the cheap content-equality checks:
    // - `cherry_merged`: rebase merge detection via `git cherry`.
    // - `trees_match`: identical tree object (cheapest content check).
    // - `diff_empty`: the branch's own commits net out to no content change.
    //
    // Effort::Thorough adds the costly ones:
    // - `patch_id_match`: commits re-applied on target with different SHAs.
    // - `merge_adds_nothing`: in-memory merge yields target's existing tree,
    //   catching squash-merges where target has since advanced.
    // - `squash_patch_id_match`: the branch's combined diff matches a single
    //   squash commit on target, surviving review-time formatting tweaks.
    const STRATEGIES: [Strategy; 6] = [
        Git::cherry_merged,
        Git::trees_match,
        Git::diff_empty,
        Git::patch_id_match,
        Git::merge_adds_nothing,
        Git::squash_patch_id_match,
    ];

    let enabled = match effort {
        Effort::Quick => 0,
        Effort::Standard => 3,
        Effort::Thorough => STRATEGIES.len(),
    };

    let mut failed_strategies: HashSet<String> = HashSet::new();

    for strategy in &STRATEGIES[..enabled] {
        for (name, reference) in pool {
            if seen.contains(name) {
                continue;
            }
            for target in targets {
                let merged = match strategy(git, target, reference) {
                    Ok(merged) => merged,
                    Err(e) => {
                        // Keep probing the remaining strategies and branches,
                        // but remember why this one could not answer. Dedupe
                        // by message so one unsupported strategy does not
                        // produce a warning per branch.
                        let msg = format!("Merge detection partially failed: {e}");
                        if failed_strategies.insert(msg.clone()) {
                            warnings.push(msg);
                        }
                        false
                    }
                };
                if merged && seen.insert(name.clone()) {
                    out.push(name.clone());
                    break;
                }
            }
        }
    }
}

/// Return local branches that are merged into *any* of the protected branches
/// and are neither protected nor ignored.
///
/// `effort` decides how many detection strategies run: see [`Effort`].
pub fn find_merged_local(git: &Git, filter: &Filter, effort: Effort) -> Result<Merged> {
    let current = git.current_branch()?;
    let targets = resolve_merge_targets(git, filter)?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut candidates: Vec<String> = Vec::new();

    for target in &targets {
        let merged = git.merged_branches(target)?;
        for branch in merged {
            if branch == current || filter.is_excluded(&branch) {
                continue;
            }
            if seen.insert(branch.clone()) {
                candidates.push(branch);
            }
        }
    }

    // Ignored branches never enter any of the content-based passes below.
    let pool: Vec<Candidate> = git
        .local_branches()?
        .into_iter()
        .filter(|b| *b != current && !filter.is_excluded(b))
        .map(|b| (b.clone(), b))
        .collect();

    let mut warnings: Vec<String> = Vec::new();
    run_strategies(
        git,
        &targets,
        &pool,
        effort,
        &mut seen,
        &mut candidates,
        &mut warnings,
    );

    candidates.sort();
    Ok(Merged {
        candidates,
        warnings,
    })
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
/// Protected and ignored branches, the current branch, and anything already
/// reported by [`find_merged_local`] (passed as `merged`) are excluded.
pub fn find_gone_local(git: &Git, filter: &Filter, merged: &[String]) -> Result<Vec<String>> {
    let current = git.current_branch()?;
    let already: HashSet<&String> = merged.iter().collect();

    let mut candidates: Vec<String> = git
        .branches_with_gone_upstream()?
        .into_iter()
        .filter(|b| *b != current && !already.contains(b) && !filter.is_excluded(b))
        .collect();

    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

/// Return remote branches that are merged into *any* of the protected branches
/// and are neither protected nor ignored, for the given remote.
///
/// Branch names are returned in short form (`feature/x`), as expected by
/// [`Git::remote_branch_delete`].
///
/// Detection mirrors [`find_merged_local`] and honours the same `effort`
/// levels, with one deliberate difference: the content-based strategies compare
/// against the **remote-tracking** counterparts of the protected branches
/// (`origin/main`), not their local ones. A branch merged into a local `main`
/// that was never pushed is not merged as far as the remote is concerned, and
/// must not be offered for deletion there.
///
/// Which branches are protected or ignored is still read from the local
/// configuration; only the refs compared against are remote.
pub fn find_merged_remote(
    git: &Git,
    filter: &Filter,
    remote: &str,
    effort: Effort,
) -> Result<Merged> {
    let targets = resolve_merge_targets(git, filter)?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut candidates: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for target in &targets {
        let merged = git.merged_remote_branches(target, remote)?;
        for branch in merged {
            if filter.is_excluded(&branch) {
                continue;
            }
            if seen.insert(branch.clone()) {
                candidates.push(branch);
            }
        }
    }

    let remote_branches = git.remote_branches(remote)?;
    let present: HashSet<&String> = remote_branches.iter().collect();

    // A protected branch that was never pushed has no remote counterpart to
    // compare against; with none at all there is nothing more to detect.
    let remote_targets: Vec<String> = targets
        .iter()
        .filter(|t| present.contains(t))
        .map(|t| format!("{remote}/{t}"))
        .collect();

    if !remote_targets.is_empty() {
        let is_target: HashSet<&String> = targets.iter().collect();
        let pool: Vec<Candidate> = remote_branches
            .iter()
            .filter(|b| !is_target.contains(b) && !filter.is_excluded(b))
            .map(|b| (b.clone(), format!("{remote}/{b}")))
            .collect();

        run_strategies(
            git,
            &remote_targets,
            &pool,
            effort,
            &mut seen,
            &mut candidates,
            &mut warnings,
        );
    }

    candidates.sort();
    Ok(Merged {
        candidates,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Create a repo with branches plus an additional `release/1.0` branch.
    fn init_repo_with_release() -> Result<(tempfile::TempDir, Git)> {
        let (dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let path = dir.path();
        Command::new("git")
            .args(["checkout", "-b", "release/1.0"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Ok((dir, git))
    }

    #[test]
    fn effort_parses_and_renders_levels() {
        assert_eq!("1".parse::<Effort>().unwrap(), Effort::Quick);
        assert_eq!(" 2 ".parse::<Effort>().unwrap(), Effort::Standard);
        assert_eq!("3".parse::<Effort>().unwrap(), Effort::Thorough);
        assert!("0".parse::<Effort>().is_err());
        assert!("4".parse::<Effort>().is_err());
        assert!("high".parse::<Effort>().is_err());

        assert_eq!(Effort::default(), Effort::Standard);
        assert_eq!(Effort::Thorough.as_u8(), 3);
        assert_eq!(Effort::Thorough.to_string(), "3 (thorough)");
        assert!(Effort::try_from(9u8).is_err());
    }

    /// Build a `Filter` for a repository/config pair.
    fn filter_for(git: &Git, config: &Config) -> Result<Filter> {
        Filter::load(git, config)
    }

    #[test]
    fn protected_patterns_match() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let filter = filter_for(&git, &config)?;
        assert!(filter.is_protected("main"));
        assert!(filter.is_protected("release/1.0"));
        assert!(filter.is_protected("release/2.0-beta"));
        assert!(!filter.is_protected("feature/foo"));
        assert!(!filter.is_protected("develop"));
        Ok(())
    }

    #[test]
    fn find_merged_local_returns_only_merged_unprotected_branches() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;

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
    fn find_gone_local_returns_branches_whose_upstream_was_deleted() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_gone_upstream()?;
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        let gone = find_gone_local(&git, &filter_for(&git, &config)?, &merged)?;

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
    fn find_gone_local_excludes_already_merged() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_gone_upstream()?;
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        // Pretend the content-based strategies already caught it.
        let merged = vec!["feature/gone".to_string()];
        assert!(find_gone_local(&git, &filter_for(&git, &config)?, &merged)?.is_empty());
        Ok(())
    }

    #[test]
    fn find_gone_local_excludes_current_and_protected() -> Result<()> {
        let (dir, git) = crate::test_helpers::init_repo_with_gone_upstream()?;

        // Protected by pattern.
        let config = Config {
            protected: vec!["main".to_string(), "feature/*".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        assert!(find_gone_local(&git, &filter_for(&git, &config)?, &[])?.is_empty());

        // Checked out: never proposed for deletion.
        Command::new("git")
            .args(["checkout", "feature/gone"])
            .current_dir(dir.path().join("work"))
            .output()?;
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        assert!(find_gone_local(&git, &filter_for(&git, &config)?, &[])?.is_empty());
        Ok(())
    }

    #[test]
    fn find_merged_local_excludes_current_branch() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        let current = git.current_branch()?;
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(!merged.contains(&current));
        Ok(())
    }

    #[test]
    fn find_merged_local_detects_cherry_picked_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch with a commit
        Command::new("git")
            .args(["checkout", "-b", "feature/cherry"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("cherry.txt"), "cherry")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "cherry feature"])
            .current_dir(path)
            .output()?;

        // Cherry-pick onto main (simulating a rebase merge)
        let log_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()?;
        let commit_sha = String::from_utf8_lossy(&log_output.stdout)
            .trim()
            .to_string();

        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;

        // Add a diverging commit on main so cherry-pick creates a distinct commit
        std::fs::write(path.join("diverge.txt"), "diverge")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "diverge"])
            .current_dir(path)
            .output()?;

        Command::new("git")
            .args(["cherry-pick", &commit_sha])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Cherry-picked branch should always be detected as merged
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let filter = filter_for(&git, &config)?;
        let merged = find_merged_local(&git, &filter, Effort::Standard)?.candidates;
        assert!(merged.contains(&"feature/cherry".to_string()));

        // At the lowest effort only ancestor merges count, so the
        // cherry-picked branch must stay untouched.
        let quick = find_merged_local(&git, &filter, Effort::Quick)?.candidates;
        assert!(
            !quick.contains(&"feature/cherry".to_string()),
            "effort 1 must not run the cherry-pick strategy"
        );
        Ok(())
    }

    #[test]
    fn find_merged_local_detects_squash_merged_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch with a commit
        Command::new("git")
            .args(["checkout", "-b", "feature/squash"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("squash.txt"), "squash")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "squash feature"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main (creates a single squash commit, not a merge commit)
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["merge", "--squash", "feature/squash"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "squash merge"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Squash-merged branch should be detected (via tree comparison here, or
        // via the patch-id / simulated-merge strategies once main advances)
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(
            merged.contains(&"feature/squash".to_string()),
            "squash-merged branch should be detected as merged"
        );
        Ok(())
    }

    #[test]
    fn find_merged_local_detects_tree_match_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch with a commit
        Command::new("git")
            .args(["checkout", "-b", "feature/tree-match"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("tree.txt"), "tree content")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "tree feature"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main so both tips share the same tree object
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["merge", "--squash", "feature/tree-match"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "squash merge tree"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Branch should be detected via tree SHA comparison
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(
            merged.contains(&"feature/tree-match".to_string()),
            "branch with matching tree SHA should be detected as merged"
        );
        Ok(())
    }

    #[test]
    fn find_merged_local_detects_patch_id_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Feature branch with one commit.
        Command::new("git")
            .args(["checkout", "-b", "feature/patch-id"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("patch.txt"), "patch content\n")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "patch-id feature"])
            .current_dir(path)
            .output()?;
        let sha = String::from_utf8_lossy(
            &Command::new("git")
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
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("diverge.txt"), "diverge\n")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "diverge"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["cherry-pick", &sha])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "--amend", "-m", "patch-id feature (reworded)"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let filter = filter_for(&git, &config)?;
        let merged = find_merged_local(&git, &filter, Effort::Thorough)?.candidates;
        assert!(
            merged.contains(&"feature/patch-id".to_string()),
            "branch with matching patch-id should be detected as merged"
        );

        // Ancestor merges only: nothing content-based runs at effort 1.
        let quick = find_merged_local(&git, &filter, Effort::Quick)?.candidates;
        assert!(
            !quick.contains(&"feature/patch-id".to_string()),
            "effort 1 must not run any content-based strategy"
        );
        Ok(())
    }

    #[test]
    fn find_merged_local_detects_simulated_merge_branches() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Create a feature branch that touches a.txt
        Command::new("git")
            .args(["checkout", "-b", "feature/squash-advanced"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("a.txt"), "feature content")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "feature: a.txt"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["merge", "--squash", "feature/squash-advanced"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "squash merge"])
            .current_dir(path)
            .output()?;

        // Advance main with an unrelated commit touching a different file,
        // so neither trees_match nor diff_empty would detect the branch.
        std::fs::write(path.join("b.txt"), "unrelated")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "main: unrelated b.txt"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Thorough)?.candidates;
        assert!(
            merged.contains(&"feature/squash-advanced".to_string()),
            "squash-merged branch with advanced target should be detected via simulated merge"
        );

        Ok(())
    }

    #[test]
    fn find_merged_local_detects_multi_commit_squash_via_patch_id() -> Result<()> {
        let (dir, _git) = crate::test_helpers::init_repo()?;
        let path = dir.path();

        // Feature branch with TWO commits — combined diff is a single
        // addition to a.txt. This is the canonical case `patch_id_match`
        // (per-commit) cannot resolve and where `merge_adds_nothing` may
        // also miss when the squashed commit on target differs textually
        // from the branch's commits.
        Command::new("git")
            .args(["checkout", "-b", "feature/multi-commit-squash"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("a.txt"), "line1\n")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "feature: line1"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("a.txt"), "line1\nline2\n")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "feature: line2"])
            .current_dir(path)
            .output()?;

        // Squash-merge onto main as a single commit.
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["merge", "--squash", "feature/multi-commit-squash"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "squash merge"])
            .current_dir(path)
            .output()?;

        // Unrelated advance on main (defeats trees_match / diff_empty).
        std::fs::write(path.join("b.txt"), "unrelated")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "main: unrelated b.txt"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let filter = filter_for(&git, &config)?;
        let merged = find_merged_local(&git, &filter, Effort::Thorough)?.candidates;
        assert!(
            merged.contains(&"feature/multi-commit-squash".to_string()),
            "multi-commit branch squash-merged into main should be detected \
             via squash patch-id"
        );

        // Combined-squash patch-id is an effort 3 strategy: the default level
        // does not pay for it, so this branch stays undetected there.
        let standard = find_merged_local(&git, &filter, Effort::Standard)?.candidates;
        assert!(
            !standard.contains(&"feature/multi-commit-squash".to_string()),
            "effort 2 must not run the squash patch-id strategy"
        );
        Ok(())
    }

    #[test]
    fn resolve_merge_targets_with_globs() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        let targets = resolve_merge_targets(&git, &filter_for(&git, &config)?)?;
        assert!(targets.contains(&"main".to_string()));
        assert!(targets.contains(&"release/1.0".to_string()));
        assert!(!targets.contains(&"feature/done".to_string()));
        assert!(!targets.contains(&"feature/wip".to_string()));
        Ok(())
    }

    #[test]
    fn find_merged_local_no_targets() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        // Use a pattern that matches nothing
        let config = Config {
            protected: vec!["nonexistent-branch".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(merged.is_empty());
        Ok(())
    }

    #[test]
    fn find_merged_local_respects_branch_protected() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        // Without per-branch protection, feature/done should be a candidate
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(merged.contains(&"feature/done".to_string()));

        // Mark feature/done as per-branch protected
        git.set_branch_protected("feature/done", true)?;
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(
            !merged.contains(&"feature/done".to_string()),
            "per-branch protected branch should not be a deletion candidate"
        );

        // Clean up
        git.set_branch_protected("feature/done", false)?;
        Ok(())
    }

    #[test]
    fn branch_protected_serves_as_merge_target() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        // Only use per-branch protection on "main" (no global patterns match anything)
        let config = Config {
            protected: vec!["nonexistent-branch".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        // Without any real protected branches, nothing is a merge target
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(merged.is_empty());

        // Mark "main" as per-branch protected — it should now be a merge target
        git.set_branch_protected("main", true)?;
        let merged =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(
            merged.contains(&"feature/done".to_string()),
            "branches merged into a per-branch protected branch should be candidates"
        );

        // Clean up
        git.set_branch_protected("main", false)?;
        Ok(())
    }

    /// Build a repo with a bare `origin`, containing a classically merged
    /// branch (`feature/merged`) and a squash-merged one (`feature/squashed`),
    /// both pushed, with `main` pushed after both merges.
    ///
    /// Returns the enclosing tempdir, the seed repo (kept alive because the
    /// bare clone was made from it), and the working clone.
    fn init_repo_with_remote_merges()
    -> Result<(tempfile::TempDir, tempfile::TempDir, std::path::PathBuf)> {
        let dir = tempfile::tempdir()?;
        let origin = dir.path().join("origin.git");
        let work = dir.path().join("work");

        use crate::test_helpers::git_in;

        let (seed, _) = crate::test_helpers::init_repo()?;
        git_in(
            seed.path(),
            &["clone", "--bare", ".", origin.to_str().unwrap()],
        )?;
        git_in(
            dir.path(),
            &["clone", origin.to_str().unwrap(), work.to_str().unwrap()],
        )?;
        git_in(&work, &["config", "user.email", "test@test.com"])?;
        git_in(&work, &["config", "user.name", "Test"])?;

        // A classically merged branch, pushed to the remote.
        git_in(&work, &["checkout", "-b", "feature/merged", "main"])?;
        std::fs::write(work.join("merged.txt"), "merged")?;
        git_in(&work, &["add", "."])?;
        git_in(&work, &["commit", "-m", "merged feature"])?;
        git_in(&work, &["push", "-u", "origin", "feature/merged"])?;

        // A squash-merged branch, also pushed: only the content-based
        // strategies can find it.
        git_in(&work, &["checkout", "-b", "feature/squashed", "main"])?;
        std::fs::write(work.join("squashed.txt"), "squashed")?;
        git_in(&work, &["add", "."])?;
        git_in(&work, &["commit", "-m", "squashed feature"])?;
        git_in(&work, &["push", "-u", "origin", "feature/squashed"])?;

        git_in(&work, &["checkout", "main"])?;
        git_in(
            &work,
            &["merge", "--no-ff", "-m", "merge", "feature/merged"],
        )?;
        git_in(&work, &["merge", "--squash", "feature/squashed"])?;
        git_in(&work, &["commit", "-m", "squash merge feature/squashed"])?;
        git_in(&work, &["push", "origin", "main"])?;
        git_in(&work, &["fetch", "origin"])?;

        Ok((dir, seed, work))
    }

    fn protected_main() -> Config {
        Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
        }
    }

    /// Remote detection runs the same strategies as local detection (#28), so a
    /// squash-merged remote branch is reported at the default effort.
    #[test]
    fn find_merged_remote_detects_squash_merges() -> Result<()> {
        let (_dir, seed, work) = init_repo_with_remote_merges()?;
        let git = Git::with_workdir(false, &work);
        let config = protected_main();

        let local =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Standard)?.candidates;
        assert!(
            local.contains(&"feature/squashed".to_string()),
            "local detection should catch the squash-merged branch"
        );

        let remote = find_merged_remote(
            &git,
            &filter_for(&git, &config)?,
            "origin",
            Effort::Standard,
        )?
        .candidates;
        assert!(
            remote.contains(&"feature/merged".to_string()),
            "remote detection should catch the classically merged branch, got {remote:?}"
        );
        assert!(
            remote.contains(&"feature/squashed".to_string()),
            "remote detection should catch the squash-merged branch, got {remote:?}"
        );
        assert!(
            !remote.contains(&"main".to_string()),
            "the protected target itself is never a candidate, got {remote:?}"
        );

        drop(seed);
        Ok(())
    }

    /// `--effort 1` keeps remote detection on `git branch -r --merged` alone.
    #[test]
    fn find_merged_remote_quick_effort_only_detects_ancestor_merges() -> Result<()> {
        let (_dir, seed, work) = init_repo_with_remote_merges()?;
        let git = Git::with_workdir(false, &work);
        let config = protected_main();

        let remote =
            find_merged_remote(&git, &filter_for(&git, &config)?, "origin", Effort::Quick)?
                .candidates;
        assert!(
            remote.contains(&"feature/merged".to_string()),
            "ancestor merges are detected at every effort level, got {remote:?}"
        );
        assert!(
            !remote.contains(&"feature/squashed".to_string()),
            "quick effort must not run the content-based strategies, got {remote:?}"
        );

        drop(seed);
        Ok(())
    }

    /// The content-based strategies compare against `origin/main`, not local
    /// `main`: a branch merged locally but not pushed is still live on the
    /// remote and must not be offered for deletion there.
    #[test]
    fn find_merged_remote_ignores_branches_merged_only_into_local_trunk() -> Result<()> {
        let (_dir, seed, work) = init_repo_with_remote_merges()?;
        use crate::test_helpers::git_in;

        git_in(&work, &["checkout", "-b", "feature/unpushed-merge", "main"])?;
        std::fs::write(work.join("unpushed.txt"), "unpushed")?;
        git_in(&work, &["add", "."])?;
        git_in(&work, &["commit", "-m", "unpushed feature"])?;
        git_in(&work, &["push", "-u", "origin", "feature/unpushed-merge"])?;

        // Squash-merged into the local trunk only — `main` is never pushed
        // again, so `origin/main` does not carry the change.
        git_in(&work, &["checkout", "main"])?;
        git_in(&work, &["merge", "--squash", "feature/unpushed-merge"])?;
        git_in(&work, &["commit", "-m", "squash merge unpushed"])?;
        git_in(&work, &["fetch", "origin"])?;

        let git = Git::with_workdir(false, &work);
        let config = protected_main();

        let local =
            find_merged_local(&git, &filter_for(&git, &config)?, Effort::Thorough)?.candidates;
        assert!(
            local.contains(&"feature/unpushed-merge".to_string()),
            "local detection compares against local main, got {local:?}"
        );

        let remote = find_merged_remote(
            &git,
            &filter_for(&git, &config)?,
            "origin",
            Effort::Thorough,
        )?
        .candidates;
        assert!(
            !remote.contains(&"feature/unpushed-merge".to_string()),
            "remote detection must compare against origin/main, got {remote:?}"
        );

        drop(seed);
        Ok(())
    }

    /// Protected and ignored patterns apply to remote branches too, including
    /// in the content-based passes.
    #[test]
    fn find_merged_remote_skips_protected_and_ignored_branches() -> Result<()> {
        let (_dir, seed, work) = init_repo_with_remote_merges()?;
        let git = Git::with_workdir(false, &work);
        let config = Config {
            protected: vec!["main".to_string(), "feature/merged".to_string()],
            ignore: vec!["feature/squash*".to_string()],
            remotes: None,
            worktrunk: None,
            effort: None,
        };

        let remote = find_merged_remote(
            &git,
            &filter_for(&git, &config)?,
            "origin",
            Effort::Thorough,
        )?
        .candidates;
        assert!(
            remote.is_empty(),
            "protected and ignored remote branches are never candidates, got {remote:?}"
        );

        drop(seed);
        Ok(())
    }

    // ── Ignored branches ─────────────────────────────────────────────

    fn config_with_ignore(ignore: &[&str]) -> Config {
        Config {
            protected: vec!["main".to_string()],
            ignore: ignore.iter().map(|s| s.to_string()).collect(),
            remotes: None,
            worktrunk: None,
            effort: None,
        }
    }

    #[test]
    fn ignored_literal_branch_is_not_a_merge_candidate() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let config = config_with_ignore(&["feature/done"]);
        let filter = filter_for(&git, &config)?;

        assert!(filter.is_ignored("feature/done"));
        assert!(
            !find_merged_local(&git, &filter, Effort::Standard)?
                .candidates
                .contains(&"feature/done".to_string())
        );
        Ok(())
    }

    #[test]
    fn ignored_glob_pattern_hides_every_matching_branch() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        let config = config_with_ignore(&["feature/*"]);
        let filter = filter_for(&git, &config)?;

        assert!(filter.is_ignored("feature/done"));
        assert!(filter.is_ignored("feature/wip"));
        assert!(!filter.is_ignored("main"));
        assert!(
            find_merged_local(&git, &filter, Effort::Standard)?
                .candidates
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn ignore_takes_precedence_over_protect() -> Result<()> {
        let (_dir, git) = init_repo_with_release()?;
        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            ignore: vec!["release/*".to_string()],
            remotes: None,
            worktrunk: None,
            effort: None,
        };
        let filter = filter_for(&git, &config)?;

        assert!(filter.is_ignored("release/1.0"));
        assert!(!filter.is_protected("release/1.0"));
        assert!(
            !resolve_merge_targets(&git, &filter)?.contains(&"release/1.0".to_string()),
            "an ignored branch must not become a merge target"
        );
        Ok(())
    }

    #[test]
    fn per_branch_ignore_flag_is_honoured() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_branches()?;
        git.set_branch_ignored("feature/done", true)?;

        let config = config_with_ignore(&[]);
        let filter = filter_for(&git, &config)?;
        assert!(filter.is_ignored("feature/done"));
        assert!(
            !find_merged_local(&git, &filter, Effort::Standard)?
                .candidates
                .contains(&"feature/done".to_string())
        );

        git.set_branch_ignored("feature/done", false)?;
        let filter = filter_for(&git, &config)?;
        assert!(!filter.is_ignored("feature/done"));
        assert!(
            find_merged_local(&git, &filter, Effort::Standard)?
                .candidates
                .contains(&"feature/done".to_string())
        );
        Ok(())
    }

    #[test]
    fn ignored_branch_is_excluded_from_gone_upstream_detection() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo_with_gone_upstream()?;
        let config = config_with_ignore(&["feature/gone"]);
        let filter = filter_for(&git, &config)?;

        assert!(find_gone_local(&git, &filter, &[])?.is_empty());
        Ok(())
    }

    #[test]
    fn ignored_branch_is_excluded_from_remote_detection() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let origin = dir.path().join("origin.git");
        let work = dir.path().join("work");

        use crate::test_helpers::git_in;

        let (seed, _) = crate::test_helpers::init_repo()?;
        git_in(
            seed.path(),
            &["clone", "--bare", ".", origin.to_str().unwrap()],
        )?;
        git_in(
            dir.path(),
            &["clone", origin.to_str().unwrap(), work.to_str().unwrap()],
        )?;
        git_in(&work, &["config", "user.email", "test@test.com"])?;
        git_in(&work, &["config", "user.name", "Test"])?;

        git_in(&work, &["checkout", "-b", "wip/spike", "main"])?;
        std::fs::write(work.join("spike.txt"), "spike")?;
        git_in(&work, &["add", "."])?;
        git_in(&work, &["commit", "-m", "spike"])?;
        git_in(&work, &["push", "-u", "origin", "wip/spike"])?;

        git_in(&work, &["checkout", "main"])?;
        git_in(&work, &["merge", "--no-ff", "-m", "merge", "wip/spike"])?;
        git_in(&work, &["push", "origin", "main"])?;
        git_in(&work, &["fetch", "origin"])?;

        let git = Git::with_workdir(false, &work);

        let visible = find_merged_remote(
            &git,
            &filter_for(&git, &config_with_ignore(&[]))?,
            "origin",
            Effort::Standard,
        )?
        .candidates;
        assert!(visible.contains(&"wip/spike".to_string()));

        let hidden = find_merged_remote(
            &git,
            &filter_for(&git, &config_with_ignore(&["wip/*"]))?,
            "origin",
            Effort::Standard,
        )?
        .candidates;
        assert!(
            !hidden.contains(&"wip/spike".to_string()),
            "ignored remote branch should be invisible, got {hidden:?}"
        );

        drop(seed);
        Ok(())
    }
}
