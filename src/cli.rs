use clap::{Parser, Subcommand};

/// Easily synchronize your local branches and worktrees.
///
/// Detects branches that have been merged into your main branch(es) and offers
/// to delete them — both locally and on configured remotes. Also handles
/// orphaned worktree cleanup.
///
/// On first run, an interactive setup wizard stores preferences in the
/// git config `[sync]` section.
#[derive(Parser, Debug)]
#[command(
    name = "git-sync",
    version,
    about,
    long_about = None,
)]
pub struct Cli {
    /// Skip all confirmation prompts (auto-confirm deletions)
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Show what would be done without actually doing it
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Show git commands being executed
    #[arg(short, long)]
    pub verbose: bool,

    /// Skip fetching and pruning remotes
    #[arg(long)]
    pub no_fetch: bool,

    /// Only clean local branches (skip remote deletion)
    #[arg(long)]
    pub local_only: bool,

    /// Only clean remote branches (skip local deletion)
    #[arg(long)]
    pub remote_only: bool,

    /// Skip worktree cleanup
    #[arg(long)]
    pub no_worktrees: bool,

    /// Use worktrunk (wt) for worktree removal to trigger pre/post-remove hooks
    #[arg(long, overrides_with = "no_worktrunk")]
    pub worktrunk: bool,

    /// Do not use worktrunk for worktree removal (overrides config)
    #[arg(long, overrides_with = "worktrunk")]
    pub no_worktrunk: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
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
        /// Configuration key (e.g. protected)
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

    /// Re-run the interactive setup wizard
    Setup,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_cli_default_flags() {
        let cli = Cli::parse_from(["git-sync"]);
        assert!(!cli.yes);
        assert!(!cli.dry_run);
        assert!(!cli.verbose);
        assert!(!cli.no_fetch);
        assert!(!cli.local_only);
        assert!(!cli.remote_only);
        assert!(!cli.no_worktrees);
        assert!(!cli.worktrunk);
        assert!(!cli.no_worktrunk);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_flag_parsing() {
        let cli = Cli::parse_from(["git-sync", "-y", "-n", "-v", "--no-fetch", "--local-only"]);
        assert!(cli.yes);
        assert!(cli.dry_run);
        assert!(cli.verbose);
        assert!(cli.no_fetch);
        assert!(cli.local_only);
        assert!(!cli.remote_only);
    }

    #[test]
    fn test_cli_worktrunk_flag() {
        let cli = Cli::parse_from(["git-sync", "--worktrunk"]);
        assert!(cli.worktrunk);
        assert!(!cli.no_worktrunk);
    }

    #[test]
    fn test_cli_no_worktrunk_flag() {
        let cli = Cli::parse_from(["git-sync", "--no-worktrunk"]);
        assert!(!cli.worktrunk);
        assert!(cli.no_worktrunk);
    }

    #[test]
    fn test_cli_worktrunk_overrides() {
        // Last flag wins with overrides_with
        let cli = Cli::parse_from(["git-sync", "--worktrunk", "--no-worktrunk"]);
        assert!(!cli.worktrunk);
        assert!(cli.no_worktrunk);

        let cli = Cli::parse_from(["git-sync", "--no-worktrunk", "--worktrunk"]);
        assert!(cli.worktrunk);
        assert!(!cli.no_worktrunk);
    }

    #[test]
    fn test_cli_config_subcommand() {
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
