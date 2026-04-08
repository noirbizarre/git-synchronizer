use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Run a git command and return its stdout as a trimmed string.
///
/// If `verbose` is true the command is printed to stderr before execution.
fn run_git(args: &[&str], verbose: bool, workdir: Option<&Path>) -> Result<String> {
    if verbose {
        eprintln!("  $ git {}", args.join(" "));
    }

    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = workdir {
        cmd.current_dir(dir);
    }

    let output = cmd
        .output()
        .with_context(|| format!("failed to execute: git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed (exit {}):\n{}",
            args.join(" "),
            output.status,
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run an external command (not git) and return its stdout as a trimmed string.
///
/// If `verbose` is true the command is printed to stderr before execution.
fn run_cmd(bin: &str, args: &[&str], verbose: bool, workdir: Option<&Path>) -> Result<String> {
    if verbose {
        eprintln!("  $ {} {}", bin, args.join(" "));
    }

    let mut cmd = Command::new(bin);
    cmd.args(args);
    if let Some(dir) = workdir {
        cmd.current_dir(dir);
    }

    let output = cmd
        .output()
        .with_context(|| format!("failed to execute: {} {}", bin, args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "{} {} failed (exit {}):\n{}",
            bin,
            args.join(" "),
            output.status,
            stderr.trim()
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Check if the worktrunk CLI (`wt`) is available on `$PATH`.
pub fn worktrunk_available() -> bool {
    Command::new("wt")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// A thin wrapper around git CLI invocations.
pub struct Git {
    verbose: bool,
    workdir: Option<std::path::PathBuf>,
}

impl Git {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            workdir: None,
        }
    }

    /// Create a Git instance that operates in a specific directory.
    #[cfg(test)]
    pub fn with_workdir(verbose: bool, workdir: &Path) -> Self {
        Self {
            verbose,
            workdir: Some(workdir.to_path_buf()),
        }
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        run_git(args, self.verbose, self.workdir.as_deref())
    }

    fn run_wt(&self, args: &[&str]) -> Result<String> {
        run_cmd("wt", args, self.verbose, self.workdir.as_deref())
    }

    // ── Repository info ──────────────────────────────────────────────

    /// Return the current branch name.
    pub fn current_branch(&self) -> Result<String> {
        self.run(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// Return the list of configured remotes.
    pub fn remotes(&self) -> Result<Vec<String>> {
        let out = self.run(&["remote"])?;
        Ok(out.lines().map(|l| l.to_string()).collect())
    }

    // ── Fetch ────────────────────────────────────────────────────────

    /// Fetch all remotes and prune deleted remote-tracking branches.
    pub fn remote_update_prune(&self) -> Result<()> {
        self.run(&["remote", "update", "--prune"])?;
        Ok(())
    }

    // ── Branch queries ───────────────────────────────────────────────

    /// Return local branches that have been merged into `target`.
    pub fn merged_branches(&self, target: &str) -> Result<Vec<String>> {
        let out = self.run(&["branch", "--merged", target])?;
        Ok(parse_branch_list(&out))
    }

    /// Return all local branch names.
    pub fn local_branches(&self) -> Result<Vec<String>> {
        let out = self.run(&["branch", "--format=%(refname:short)"])?;
        Ok(out
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect())
    }

    /// Return remote-tracking branches merged into `target` for the given remote.
    pub fn merged_remote_branches(&self, target: &str, remote: &str) -> Result<Vec<String>> {
        let out = self.run(&["branch", "-r", "--merged", target])?;
        let prefix = format!("{remote}/");
        Ok(out
            .lines()
            .map(|l| l.trim())
            .filter(|l| l.starts_with(&prefix) && !l.contains("->"))
            .map(|l| l.strip_prefix(&prefix).unwrap_or(l).to_string())
            .collect())
    }

    /// Use `git cherry` to detect rebase-merged branches.
    ///
    /// Returns branch names whose commits have all been applied upstream.
    pub fn cherry_merged(&self, upstream: &str, branch: &str) -> Result<bool> {
        let out = self.run(&["cherry", upstream, branch])?;
        // If all lines start with `-`, every commit was cherry-picked upstream.
        Ok(!out.is_empty() && out.lines().all(|l| l.starts_with('-')))
    }

    // ── Branch mutations ─────────────────────────────────────────────

    /// Delete a local branch.
    pub fn branch_delete(&self, branch: &str) -> Result<()> {
        self.run(&["branch", "-d", branch])?;
        Ok(())
    }

    /// Delete a branch on a remote (with --force-with-lease for safety).
    pub fn push_delete(&self, remote: &str, branch: &str) -> Result<()> {
        self.run(&["push", "--delete", "--force-with-lease", remote, branch])?;
        Ok(())
    }

    // ── Worktree operations ──────────────────────────────────────────

    /// Parsed worktree entry from `git worktree list --porcelain`.
    pub fn worktree_list(&self) -> Result<Vec<Worktree>> {
        let out = self.run(&["worktree", "list", "--porcelain"])?;
        Ok(parse_worktree_list(&out))
    }

    /// Remove a worktree by path.
    pub fn worktree_remove(&self, path: &str) -> Result<()> {
        self.run(&["worktree", "remove", path])?;
        Ok(())
    }

    // ── Worktrunk integration ────────────────────────────────────────

    /// Check if a worktrunk config section exists in git config.
    ///
    /// Worktrunk stores its state under the `[worktrunk]` git config section.
    /// Its presence indicates the repository is managed by worktrunk.
    pub fn worktrunk_config_exists(&self) -> Result<bool> {
        self.config_section_exists("worktrunk")
    }

    /// Remove a worktree via the worktrunk CLI, triggering pre/post-remove hooks.
    ///
    /// Uses `--foreground` to wait for hooks to complete, `--yes` to skip
    /// wt's approval prompts (git-sync already confirmed with the user), and
    /// `--no-delete-branch` because git-sync manages branch deletion separately.
    pub fn worktrunk_remove(&self, branch: &str) -> Result<()> {
        self.run_wt(&[
            "remove",
            branch,
            "--foreground",
            "--yes",
            "--no-delete-branch",
        ])?;
        Ok(())
    }

    /// Remove a worktree via the worktrunk CLI using its path.
    ///
    /// Used for detached HEAD worktrees or orphans where the branch name
    /// is not available. Falls back to path-based removal.
    pub fn worktrunk_remove_by_path(&self, path: &str) -> Result<()> {
        self.run_wt(&[
            "remove",
            path,
            "--foreground",
            "--yes",
            "--no-delete-branch",
        ])?;
        Ok(())
    }

    // ── Config operations ────────────────────────────────────────────

    /// Get all values for a multi-valued config key.
    pub fn config_get_all(&self, key: &str) -> Result<Vec<String>> {
        match self.run(&["config", "--get-all", key]) {
            Ok(out) => Ok(out
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect()),
            Err(_) => Ok(vec![]),
        }
    }

    /// Get a single config value.
    pub fn config_get(&self, key: &str) -> Result<Option<String>> {
        match self.run(&["config", "--get", key]) {
            Ok(val) if !val.is_empty() => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    /// Set a single-valued config key.
    pub fn config_set(&self, key: &str, value: &str) -> Result<()> {
        self.run(&["config", key, value])?;
        Ok(())
    }

    /// Add a value to a multi-valued config key.
    pub fn config_add(&self, key: &str, value: &str) -> Result<()> {
        self.run(&["config", "--add", key, value])?;
        Ok(())
    }

    /// Remove all values for a config key.
    pub fn config_unset_all(&self, key: &str) -> Result<()> {
        // --unset-all exits non-zero if the key doesn't exist; that's fine.
        let _ = self.run(&["config", "--unset-all", key]);
        Ok(())
    }

    /// Check whether a config section exists.
    pub fn config_section_exists(&self, section: &str) -> Result<bool> {
        match self.run(&["config", "--get-regexp", &format!("^{section}\\.")]) {
            Ok(out) => Ok(!out.is_empty()),
            Err(_) => Ok(false),
        }
    }

    // ── Per-branch protection ────────────────────────────────────────

    /// Return the names of branches that have
    /// `branch.<name>.sync-protected = true` in git config.
    pub fn branch_protected_list(&self) -> Result<Vec<String>> {
        let pattern = r"^branch\..*\.sync-protected$";
        match self.run(&["config", "--get-regexp", pattern]) {
            Ok(out) => {
                let mut branches = Vec::new();
                for line in out.lines().filter(|l| !l.is_empty()) {
                    // Each line: "branch.<name>.sync-protected true"
                    let mut parts = line.splitn(2, ' ');
                    if let (Some(key), Some(value)) = (parts.next(), parts.next())
                        && value.trim().eq_ignore_ascii_case("true")
                    {
                        // Extract branch name from "branch.<name>.sync-protected"
                        if let Some(name) = key
                            .strip_prefix("branch.")
                            .and_then(|s| s.strip_suffix(".sync-protected"))
                        {
                            branches.push(name.to_string());
                        }
                    }
                }
                Ok(branches)
            }
            Err(_) => Ok(vec![]),
        }
    }

    /// Set or unset per-branch protection for a given branch.
    ///
    /// When `protected` is `true`, sets `branch.<name>.sync-protected = true`.
    /// When `false`, unsets the key entirely.
    pub fn set_branch_protected(&self, branch: &str, protected: bool) -> Result<()> {
        let key = format!("branch.{branch}.sync-protected");
        if protected {
            self.run(&["config", &key, "true"])?;
        } else {
            // --unset exits non-zero if the key doesn't exist; that's fine.
            let _ = self.run(&["config", "--unset", &key]);
        }
        Ok(())
    }
}

// ── Parsing helpers ──────────────────────────────────────────────────

/// A worktree entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub is_bare: bool,
}

/// Parse `git branch` output (with leading `*`, `+` and whitespace).
///
/// `*` marks the current branch, `+` marks branches checked out in
/// other linked worktrees — both are stripped.
fn parse_branch_list(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('*'))
        .map(|l| l.strip_prefix("+ ").unwrap_or(l).to_string())
        .collect()
}

/// Parse `git worktree list --porcelain` output.
fn parse_worktree_list(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current: Option<Worktree> = None;

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(wt) = current.take() {
                worktrees.push(wt);
            }
            current = Some(Worktree {
                path: path.to_string(),
                head: None,
                branch: None,
                is_bare: false,
            });
        } else if let Some(head) = line.strip_prefix("HEAD ") {
            if let Some(ref mut wt) = current {
                wt.head = Some(head.to_string());
            }
        } else if let Some(branch) = line.strip_prefix("branch ") {
            if let Some(ref mut wt) = current {
                // Strip refs/heads/ prefix
                wt.branch = Some(
                    branch
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch)
                        .to_string(),
                );
            }
        } else if line == "bare"
            && let Some(ref mut wt) = current
        {
            wt.is_bare = true;
        }
    }

    if let Some(wt) = current {
        worktrees.push(wt);
    }

    worktrees
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_branch_list() {
        let output = "  feature/foo\n* main\n  bugfix/bar\n";
        let branches = parse_branch_list(output);
        assert_eq!(branches, vec!["feature/foo", "bugfix/bar"]);
    }

    #[test]
    fn test_parse_branch_list_strips_worktree_marker() {
        let output = "  feature/foo\n* main\n+ feature/wt\n  bugfix/bar\n";
        let branches = parse_branch_list(output);
        assert_eq!(branches, vec!["feature/foo", "feature/wt", "bugfix/bar"]);
    }

    #[test]
    fn test_parse_branch_list_empty() {
        let branches = parse_branch_list("");
        assert!(branches.is_empty());
    }

    #[test]
    fn test_parse_worktree_list() {
        let output = "\
worktree /home/user/project
HEAD abc1234
branch refs/heads/main

worktree /home/user/project-feature
HEAD def5678
branch refs/heads/feature/foo

worktree /home/user/project-bare
HEAD 000000
bare
";
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 3);

        assert_eq!(worktrees[0].path, "/home/user/project");
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert!(!worktrees[0].is_bare);

        assert_eq!(worktrees[1].path, "/home/user/project-feature");
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature/foo"));

        assert_eq!(worktrees[2].path, "/home/user/project-bare");
        assert!(worktrees[2].is_bare);
    }

    #[test]
    fn test_parse_worktree_list_empty() {
        let worktrees = parse_worktree_list("");
        assert!(worktrees.is_empty());
    }

    /// Integration test: verify basic git operations in a temporary repo.
    #[test]
    fn test_git_in_temp_repo() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        // Initialize a bare-minimum repo
        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;

        // Create an initial commit
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Test current branch
        assert_eq!(git.current_branch()?, "main");

        // Test local branches
        let branches = git.local_branches()?;
        assert_eq!(branches, vec!["main"]);

        // Create a feature branch and merge it
        Command::new("git")
            .args(["checkout", "-b", "feature/test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("feature.txt"), "feature")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "feature"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["merge", "feature/test"])
            .current_dir(path)
            .output()?;

        // The feature branch should show up as merged
        let merged = git.merged_branches("main")?;
        assert!(merged.contains(&"feature/test".to_string()));

        // Config operations
        git.config_add("sync.protected", "main")?;
        git.config_add("sync.protected", "release/*")?;
        let protected = git.config_get_all("sync.protected")?;
        assert_eq!(protected, vec!["main", "release/*"]);

        assert!(git.config_section_exists("sync")?);
        assert!(!git.config_section_exists("nonexistent")?);

        Ok(())
    }

    #[test]
    fn test_branch_delete() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        // Create and merge a branch
        Command::new("git")
            .args(["checkout", "-b", "feature/to-delete"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("f.txt"), "feature")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "feature"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["merge", "feature/to-delete"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        let branches = git.local_branches()?;
        assert!(branches.contains(&"feature/to-delete".to_string()));

        git.branch_delete("feature/to-delete")?;

        let branches = git.local_branches()?;
        assert!(!branches.contains(&"feature/to-delete".to_string()));

        Ok(())
    }

    #[test]
    fn test_remotes_empty() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let remotes = git.remotes()?;
        assert!(remotes.is_empty());

        Ok(())
    }

    #[test]
    fn test_cherry_merged() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        // Create a feature branch
        Command::new("git")
            .args(["checkout", "-b", "feature/cherry-test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("cherry.txt"), "cherry content")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "cherry commit"])
            .current_dir(path)
            .output()?;
        let sha_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()?;
        let sha = String::from_utf8_lossy(&sha_output.stdout)
            .trim()
            .to_string();

        // Add a diverging commit on main so cherry-pick creates a different SHA
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("diverge.txt"), "diverge")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "diverge"])
            .current_dir(path)
            .output()?;

        // Cherry-pick the feature commit onto the now-diverged main
        Command::new("git")
            .args(["cherry-pick", &sha])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // The branch's commit was cherry-picked, so cherry_merged should be true
        assert!(git.cherry_merged("main", "feature/cherry-test")?);

        // Create an unmerged branch
        Command::new("git")
            .args(["checkout", "-b", "feature/not-cherry"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("not-cherry.txt"), "not cherry")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "not cherry-picked"])
            .current_dir(path)
            .output()?;

        // This branch's commit was NOT cherry-picked, so cherry_merged should be false
        assert!(!git.cherry_merged("main", "feature/not-cherry")?);

        Ok(())
    }

    #[test]
    fn test_worktree_list_integration() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        // Create a branch and worktree
        Command::new("git")
            .args(["branch", "feature/wt"])
            .current_dir(path)
            .output()?;
        let wt_path = path.join("wt-dir");
        Command::new("git")
            .args(["worktree", "add", wt_path.to_str().unwrap(), "feature/wt"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let worktrees = git.worktree_list()?;

        // Should have at least 2 worktrees: main repo + the added one
        assert!(worktrees.len() >= 2);

        let wt_branches: Vec<Option<&str>> =
            worktrees.iter().map(|wt| wt.branch.as_deref()).collect();
        assert!(wt_branches.contains(&Some("main")));
        assert!(wt_branches.contains(&Some("feature/wt")));

        Ok(())
    }

    #[test]
    fn test_branch_protected_list_empty() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let protected = git.branch_protected_list()?;
        assert!(protected.is_empty());

        Ok(())
    }

    #[test]
    fn test_branch_protected_list() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        // Mark two branches as protected via per-branch config
        Command::new("git")
            .args(["config", "branch.develop.sync-protected", "true"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "branch.staging.sync-protected", "true"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);
        let mut protected = git.branch_protected_list()?;
        protected.sort();
        assert_eq!(protected, vec!["develop", "staging"]);

        Ok(())
    }

    #[test]
    fn test_set_branch_protected_and_unset() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path();

        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()?;
        std::fs::write(path.join("README.md"), "# test")?;
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()?;
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()?;

        let git = Git::with_workdir(false, path);

        // Set protection
        git.set_branch_protected("develop", true)?;
        let protected = git.branch_protected_list()?;
        assert_eq!(protected, vec!["develop"]);

        // Unset protection
        git.set_branch_protected("develop", false)?;
        let protected = git.branch_protected_list()?;
        assert!(protected.is_empty());

        // Unsetting a non-existent key should not error
        git.set_branch_protected("nonexistent", false)?;

        Ok(())
    }
}
