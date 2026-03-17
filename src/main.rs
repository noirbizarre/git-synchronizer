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
                    ui.heading("Current configuration [merge-cleaner]:");
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
                }
                None => {
                    ui.muted("No configuration found. Run `git mc` to start the setup wizard.");
                }
            }
            Ok(())
        }

        ConfigAction::Set { key, value } => {
            let full_key = format!("merge-cleaner.{key}");
            git.config_set(&full_key, &value)?;
            ui.success(&format!("Set {key} = {value}"));
            Ok(())
        }

        ConfigAction::AddProtected { pattern } => {
            git.config_add("merge-cleaner.protected", &pattern)?;
            ui.success(&format!("Added protected pattern: {pattern}"));
            Ok(())
        }

        ConfigAction::RemoveProtected { pattern } => {
            let mut protected = git.config_get_all("merge-cleaner.protected")?;
            protected.retain(|p| p != &pattern);
            git.config_unset_all("merge-cleaner.protected")?;
            for p in &protected {
                git.config_add("merge-cleaner.protected", p)?;
            }
            ui.success(&format!("Removed protected pattern: {pattern}"));
            Ok(())
        }

        ConfigAction::AddRemote { name } => {
            git.config_add("merge-cleaner.remote", &name)?;
            ui.success(&format!("Added remote: {name}"));
            Ok(())
        }

        ConfigAction::RemoveRemote { name } => {
            let mut remotes = git.config_get_all("merge-cleaner.remote")?;
            remotes.retain(|r| r != &name);
            git.config_unset_all("merge-cleaner.remote")?;
            for r in &remotes {
                git.config_add("merge-cleaner.remote", r)?;
            }
            ui.success(&format!("Removed remote: {name}"));
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

    let opts = cleaner::CleanerOptions {
        yes: cli.yes,
        dry_run: cli.dry_run,
        no_fetch: cli.no_fetch,
        local_only: cli.local_only,
        remote_only: cli.remote_only,
        no_worktrees: cli.no_worktrees,
    };

    cleaner::run(git, &cfg, ui, &opts)
}
