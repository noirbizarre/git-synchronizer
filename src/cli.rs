//! Command-line surface: flags, subcommands and their help text.
//!
//! Parsed with clap into [`Cli`] and [`ConfigAction`], which `main` dispatches.

use clap::{Parser, Subcommand};

use crate::duration::MinAge;

/// Easily synchronize your local branches and worktrees.
///
/// Detects branches that have been merged into your main branch(es) and offers
/// to delete them — both locally and on configured remotes. Also handles
/// orphaned worktree cleanup.
///
/// On first run, an interactive setup wizard stores preferences in the
/// git config `[sync]` section.
#[derive(Parser, Debug)]
#[command(name = "git-sync", version, about, long_about)]
pub struct Cli {
    /// Skip all confirmation prompts (auto-confirm deletions)
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Force-remove worktrees with uncommitted changes or unmerged commits
    ///
    /// Without this flag the forced-removal prompt defaults to nothing
    /// selected, and with --yes (or --json) problematic worktrees are skipped
    /// entirely. Interactively, --force pre-selects them; you can still
    /// uncheck any entry.
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Show what would be done without actually doing it
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Show git commands being executed
    #[arg(short, long)]
    pub verbose: bool,

    /// Skip fetching and pruning remotes
    #[arg(long)]
    pub no_fetch: bool,

    /// Skip pulling (fast-forwarding) target branches before detection
    #[arg(long)]
    pub no_pull: bool,

    /// Only clean local branches (skip remote deletion)
    #[arg(long)]
    pub local_only: bool,

    /// Only clean remote branches (skip local deletion)
    #[arg(long)]
    pub remote_only: bool,

    /// Skip worktree cleanup
    #[arg(long)]
    pub no_worktrees: bool,

    /// With --yes, also delete branches whose upstream branch was deleted
    ///
    /// These branches are always listed for interactive selection, but are
    /// never pre-selected because a deleted upstream does not prove the branch
    /// was merged. Requires up-to-date remote-tracking refs, so it has no
    /// effect when --no-fetch is used outside of --dry-run.
    #[arg(long)]
    pub delete_gone: bool,

    /// How thorough merge detection should be (default: 2)
    ///
    /// Levels are cumulative:
    ///
    /// 1: ancestor merges only (`git branch --merged`), fastest.
    ///
    /// 2: adds cherry-pick, tree SHA and empty diff detection.
    ///
    /// 3: adds patch-id, simulated merge and squash patch-id detection,
    /// the most thorough but noticeably slower.
    #[arg(long, value_name = "LEVEL", value_parser = clap::value_parser!(u8).range(1..=3))]
    pub effort: Option<u8>,

    /// Skip worktrees created less than this long ago (default: 0s)
    ///
    /// Accepts a single value and unit: 30s, 15m, 2h, 7d, 1w — or a bare 0 to
    /// disable the guard. Protects a worktree you just created from the
    /// default branch from being removed along with its "merged" branch.
    #[arg(long, value_name = "DURATION")]
    pub min_age: Option<MinAge>,

    /// Use worktrunk (wt) for worktree removal to trigger pre/post-remove hooks
    #[arg(long, overrides_with = "no_worktrunk")]
    pub worktrunk: bool,

    /// Do not use worktrunk for worktree removal (overrides config)
    #[arg(long, overrides_with = "worktrunk")]
    pub no_worktrunk: bool,

    /// Output a single JSON document to stdout (implies --yes)
    ///
    /// Human-readable logs keep going to stderr. The document is pretty-printed
    /// on a terminal and compact when piped or redirected.
    ///
    /// Global so it can be given before or after a subcommand
    /// (`git sync config list --json`).
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Cli {
    /// Whether prompts should be skipped.
    ///
    /// JSON output implies `--yes`: prompts would corrupt the document and
    /// hang non-interactive callers.
    pub fn effective_yes(&self) -> bool {
        self.yes || self.json
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Manage git-sync configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Display current configuration
    List,

    /// Set a configuration value
    Set {
        /// Configuration key (e.g. worktrunk)
        ///
        /// For multi-valued keys (protected, ignore, remote) this replaces
        /// every existing value; use the add-*/remove-* subcommands to edit
        /// them individually.
        key: String,
        /// Value to set
        value: String,
    },

    /// Add a protected branch pattern
    AddProtected {
        /// Glob pattern (e.g. release/*)
        pattern: String,
    },

    /// Remove a protected branch pattern
    RemoveProtected {
        /// Glob pattern to remove
        pattern: String,
    },

    /// Add an ignored branch pattern
    ///
    /// Ignored branches are not fetched and are invisible to every detection
    /// pass. Ignoring takes precedence over protection.
    AddIgnore {
        /// Glob pattern (e.g. wip/*)
        pattern: String,
    },

    /// Remove an ignored branch pattern
    RemoveIgnore {
        /// Glob pattern to remove
        pattern: String,
    },

    /// Add a remote to operate on
    AddRemote {
        /// Remote name (e.g. origin)
        name: String,
    },

    /// Remove a remote from the configured list
    RemoveRemote {
        /// Remote name to remove
        name: String,
    },

    /// Mark a branch as protected via per-branch config
    Protect {
        /// Branch name to protect
        branch: String,
    },

    /// Remove per-branch protection from a branch
    Unprotect {
        /// Branch name to unprotect
        branch: String,
    },

    /// Mark a branch as ignored via per-branch config
    Ignore {
        /// Branch name to ignore
        branch: String,
    },

    /// Remove the per-branch ignore flag from a branch
    Unignore {
        /// Branch name to stop ignoring
        branch: String,
    },

    /// Re-run the interactive setup wizard
    Setup,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_default_flags() {
        let cli = Cli::parse_from(["git-sync"]);
        assert!(!cli.yes);
        assert!(!cli.force);
        assert!(!cli.dry_run);
        assert!(!cli.verbose);
        assert!(!cli.no_fetch);
        assert!(!cli.no_pull);
        assert!(!cli.local_only);
        assert!(!cli.remote_only);
        assert!(!cli.no_worktrees);
        assert!(!cli.delete_gone);
        assert!(!cli.worktrunk);
        assert!(!cli.no_worktrunk);
        assert!(cli.effort.is_none());
        assert!(!cli.json);
        assert!(!cli.effective_yes());
        assert!(cli.command.is_none());
    }

    #[test]
    fn cli_json_flag_implies_yes() {
        let cli = Cli::parse_from(["git-sync", "--json"]);
        assert!(cli.json);
        assert!(!cli.yes);
        assert!(cli.effective_yes());
        // --json implies --yes, but never --force: forced removal of dirty
        // worktrees stays an explicit opt-in.
        assert!(!cli.force);
    }

    #[test]
    fn cli_effective_yes_follows_yes_flag() {
        let cli = Cli::parse_from(["git-sync", "-y"]);
        assert!(!cli.json);
        assert!(cli.effective_yes());
    }

    #[test]
    fn cli_flag_parsing() {
        let cli = Cli::parse_from([
            "git-sync",
            "-y",
            "-f",
            "-n",
            "-v",
            "--no-fetch",
            "--local-only",
        ]);
        assert!(cli.yes);
        assert!(cli.force);
        assert!(cli.dry_run);
        assert!(cli.verbose);
        assert!(cli.no_fetch);
        assert!(cli.local_only);
        assert!(!cli.remote_only);
    }

    #[test]
    fn cli_force_flag() {
        assert!(Cli::parse_from(["git-sync", "--force"]).force);
        assert!(Cli::parse_from(["git-sync", "-f"]).force);
        // --force does not auto-confirm the main selection prompt.
        assert!(!Cli::parse_from(["git-sync", "--force"]).effective_yes());
    }

    #[test]
    fn cli_no_pull_flag() {
        let cli = Cli::parse_from(["git-sync", "--no-pull"]);
        assert!(cli.no_pull);
        assert!(!cli.no_fetch);
    }

    #[test]
    fn cli_effort_flag() {
        assert_eq!(
            Cli::parse_from(["git-sync", "--effort", "1"]).effort,
            Some(1)
        );
        assert_eq!(
            Cli::parse_from(["git-sync", "--effort", "3"]).effort,
            Some(3)
        );
    }

    #[test]
    fn cli_effort_rejects_out_of_range_levels() {
        assert!(Cli::try_parse_from(["git-sync", "--effort", "0"]).is_err());
        assert!(Cli::try_parse_from(["git-sync", "--effort", "4"]).is_err());
        assert!(Cli::try_parse_from(["git-sync", "--effort", "max"]).is_err());
    }

    #[test]
    fn cli_min_age_flag() {
        let cli = Cli::parse_from(["git-sync", "--min-age", "2h"]);
        assert_eq!(cli.min_age, Some("2h".parse().unwrap()));
        assert_eq!(Cli::parse_from(["git-sync"]).min_age, None);
    }

    #[test]
    fn cli_min_age_rejects_garbage() {
        assert!(Cli::try_parse_from(["git-sync", "--min-age", "soon"]).is_err());
        assert!(Cli::try_parse_from(["git-sync", "--min-age", "5x"]).is_err());
    }

    #[test]
    fn cli_worktrunk_flag() {
        let cli = Cli::parse_from(["git-sync", "--worktrunk"]);
        assert!(cli.worktrunk);
        assert!(!cli.no_worktrunk);
    }

    #[test]
    fn cli_no_worktrunk_flag() {
        let cli = Cli::parse_from(["git-sync", "--no-worktrunk"]);
        assert!(!cli.worktrunk);
        assert!(cli.no_worktrunk);
    }

    #[test]
    fn cli_worktrunk_overrides() {
        // Last flag wins with overrides_with
        let cli = Cli::parse_from(["git-sync", "--worktrunk", "--no-worktrunk"]);
        assert!(!cli.worktrunk);
        assert!(cli.no_worktrunk);

        let cli = Cli::parse_from(["git-sync", "--no-worktrunk", "--worktrunk"]);
        assert!(cli.worktrunk);
        assert!(!cli.no_worktrunk);
    }

    #[test]
    fn cli_config_subcommand() {
        let cli = Cli::parse_from(["git-sync", "config", "list"]);
        assert!(cli.command.is_some());
        match cli.command.unwrap() {
            Command::Config { action } => match action {
                ConfigAction::List => {} // expected
                _ => panic!("Expected ConfigAction::List"),
            },
        }

        let cli = Cli::parse_from(["git-sync", "config", "set", "remote", "origin"]);
        match cli.command.unwrap() {
            Command::Config { action } => match action {
                ConfigAction::Set { key, value } => {
                    assert_eq!(key, "remote");
                    assert_eq!(value, "origin");
                }
                _ => panic!("Expected ConfigAction::Set"),
            },
        }

        let cli = Cli::parse_from(["git-sync", "config", "add-protected", "release/*"]);
        match cli.command.unwrap() {
            Command::Config { action } => match action {
                ConfigAction::AddProtected { pattern } => {
                    assert_eq!(pattern, "release/*");
                }
                _ => panic!("Expected ConfigAction::AddProtected"),
            },
        }
    }
}
