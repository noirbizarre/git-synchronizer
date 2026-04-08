mod branches;
mod cleaner;
mod cli;
mod config;
mod git;
mod ui;
mod worktrees;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command, ConfigAction};

fn main() -> Result<()> {
    let cli = Cli::parse();

    let git = git::Git::new(cli.verbose);
    let ui = ui::Ui::new();

    match cli.command {
        Some(Command::Config { action }) => handle_config_command(&git, &ui, action),
        None => handle_clean(&git, &ui, &cli),
    }
}

fn handle_config_command(git: &git::Git, ui: &ui::Ui, action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::List => {
            match config::Config::load(git)? {
                Some(cfg) => {
                    ui.heading("Current configuration [sync]:");
                    ui.blank();

                    ui.line(&format!(
                        "  {} {}",
                        ui.bold.apply_to("protected:"),
                        if cfg.protected.is_empty() {
                            "(none)".to_string()
                        } else {
                            cfg.protected.join(", ")
                        }
                    ));
                    ui.line(&format!(
                        "  {} {}",
                        ui.bold.apply_to("remotes:"),
                        match &cfg.remotes {
                            Some(r) => r.join(", "),
                            None => "(all)".to_string(),
                        }
                    ));

                    let branch_protected = git.branch_protected_list()?;
                    ui.line(&format!(
                        "  {} {}",
                        ui.bold.apply_to("branch protected:"),
                        if branch_protected.is_empty() {
                            "(none)".to_string()
                        } else {
                            branch_protected.join(", ")
                        }
                    ));

                    ui.line(&format!(
                        "  {} {}",
                        ui.bold.apply_to("worktrunk:"),
                        match cfg.worktrunk {
                            Some(true) => "enabled",
                            Some(false) => "disabled",
                            None => "(auto-detect)",
                        }
                    ));
                }
                None => {
                    ui.muted("No configuration found. Run `git sync` to start the setup wizard.");
                }
            }
            Ok(())
        }

        ConfigAction::Set { key, value } => {
            let full_key = format!("sync.{key}");
            git.config_set(&full_key, &value)?;
            ui.success(&format!("Set {key} = {value}"));
            Ok(())
        }

        ConfigAction::AddProtected { pattern } => {
            git.config_add("sync.protected", &pattern)?;
            ui.success(&format!("Added protected pattern: {pattern}"));
            Ok(())
        }

        ConfigAction::RemoveProtected { pattern } => {
            let mut protected = git.config_get_all("sync.protected")?;
            protected.retain(|p| p != &pattern);
            git.config_unset_all("sync.protected")?;
            for p in &protected {
                git.config_add("sync.protected", p)?;
            }
            ui.success(&format!("Removed protected pattern: {pattern}"));
            Ok(())
        }

        ConfigAction::AddRemote { name } => {
            git.config_add("sync.remote", &name)?;
            ui.success(&format!("Added remote: {name}"));
            Ok(())
        }

        ConfigAction::RemoveRemote { name } => {
            let mut remotes = git.config_get_all("sync.remote")?;
            remotes.retain(|r| r != &name);
            git.config_unset_all("sync.remote")?;
            for r in &remotes {
                git.config_add("sync.remote", r)?;
            }
            ui.success(&format!("Removed remote: {name}"));
            Ok(())
        }

        ConfigAction::Protect { branch } => {
            git.set_branch_protected(&branch, true)?;
            ui.success(&format!("Branch '{branch}' marked as protected"));
            Ok(())
        }

        ConfigAction::Unprotect { branch } => {
            git.set_branch_protected(&branch, false)?;
            ui.success(&format!("Branch '{branch}' is no longer protected"));
            Ok(())
        }

        ConfigAction::Setup => {
            config::Config::interactive_setup(git, ui)?;
            Ok(())
        }
    }
}

fn handle_clean(git: &git::Git, ui: &ui::Ui, cli: &Cli) -> Result<()> {
    let cfg = config::load_or_setup(git, ui)?;

    let use_worktrunk = resolve_worktrunk(git, ui, cli, &cfg)?;

    let opts = cleaner::CleanerOptions {
        yes: cli.yes,
        dry_run: cli.dry_run,
        no_fetch: cli.no_fetch,
        local_only: cli.local_only,
        remote_only: cli.remote_only,
        no_worktrees: cli.no_worktrees,
        use_worktrunk,
    };

    cleaner::run(git, &cfg, ui, &opts)
}

/// Resolve whether to use worktrunk for worktree removal.
///
/// Priority: CLI flag > config setting > auto-detect from worktrunk config presence.
fn resolve_worktrunk(git: &git::Git, ui: &ui::Ui, cli: &Cli, cfg: &config::Config) -> Result<bool> {
    // 1. Explicit CLI flags take highest priority
    if cli.worktrunk {
        if !git::worktrunk_available() {
            anyhow::bail!(
                "Worktrunk (wt) not found on $PATH. \
                 Install it from https://worktrunk.dev or remove --worktrunk."
            );
        }
        return Ok(true);
    }
    if cli.no_worktrunk {
        return Ok(false);
    }

    // 2. Config setting
    if let Some(val) = cfg.worktrunk {
        if val && !git::worktrunk_available() {
            anyhow::bail!(
                "sync.worktrunk is enabled but worktrunk (wt) is not found on $PATH. \
                 Install it from https://worktrunk.dev or run: \
                 git sync config set worktrunk false"
            );
        }
        return Ok(val);
    }

    // 3. Auto-detect: check if worktrunk config section exists in git config
    if git.worktrunk_config_exists()? && git::worktrunk_available() {
        if cli.yes {
            return Ok(true);
        }
        return ui.confirm(
            "Worktrunk detected. Use it for worktree removal (triggers pre/post-remove hooks)?",
            true,
        );
    }

    Ok(false)
}
