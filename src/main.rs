mod branches;
mod cleaner;
mod cli;
mod config;
mod git;
#[cfg(test)]
mod test_helpers;
mod ui;
mod worktrees;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command, ConfigAction};
use git::{GitCommandError, GitErrorKind};

fn main() -> ExitCode {
    let cli = Cli::parse();

    let git = git::Git::new(cli.verbose);
    let ui = ui::Ui::new();

    match git.is_inside_work_tree() {
        Ok(true) => {}
        Ok(false) => {
            ui.error("Not a git repository (or any of the parent directories).");
            return ExitCode::FAILURE;
        }
        Err(err) => {
            report_error(&ui, &err);
            return ExitCode::FAILURE;
        }
    }

    let result = match cli.command {
        Some(Command::Config { action }) => handle_config_command(&git, &ui, action),
        None => handle_clean(&git, &ui, &cli),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if is_cancelled(&err) => {
            ui.muted("Cancelled.");
            ExitCode::FAILURE
        }
        Err(err) => {
            report_error(&ui, &err);
            ExitCode::FAILURE
        }
    }
}

/// Whether an error chain represents a user-cancelled prompt (Esc / Ctrl-C).
///
/// Interactive prompts surface cancellation as an `io::ErrorKind::Interrupted`
/// error; we treat it as a clean abort rather than a failure to report.
fn is_cancelled(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::Interrupted)
    })
}

/// Render an `anyhow::Error` chain using the same UI styling as the rest of
/// the CLI, without ever printing a Rust stack backtrace.
///
/// When the root cause is a [`GitCommandError`] classified as a network or
/// auth failure, the headline gets a matching prefix so users can tell at a
/// glance "this is my network, not a bug".
fn report_error(ui: &ui::Ui, err: &anyhow::Error) {
    let headline = if let Some(gerr) = err.downcast_ref::<GitCommandError>() {
        let cause = gerr.short_cause();
        match gerr.kind {
            GitErrorKind::Network => format!("Network error: {cause}"),
            GitErrorKind::Auth => format!("Authentication error: {cause}"),
            GitErrorKind::Other => format!("{err}"),
        }
    } else {
        format!("{err}")
    };

    ui.error(&headline);

    // Print the rest of the causal chain (skipping the headline we already
    // rendered). Each cause is dimmed for readability.
    for cause in err.chain().skip(1) {
        ui.muted(&format!("  caused by: {cause}"));
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
                        ui.bold_style.apply_to("protected:"),
                        if cfg.protected.is_empty() {
                            "(none)".to_string()
                        } else {
                            cfg.protected.join(", ")
                        }
                    ));
                    ui.line(&format!(
                        "  {} {}",
                        ui.bold_style.apply_to("remotes:"),
                        match &cfg.remotes {
                            Some(r) => r.join(", "),
                            None => "(all)".to_string(),
                        }
                    ));

                    let branch_protected = git.branch_protected_list()?;
                    ui.line(&format!(
                        "  {} {}",
                        ui.bold_style.apply_to("branch protected:"),
                        if branch_protected.is_empty() {
                            "(none)".to_string()
                        } else {
                            branch_protected.join(", ")
                        }
                    ));

                    ui.line(&format!(
                        "  {} {}",
                        ui.bold_style.apply_to("worktrunk:"),
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
            let full_key = format!("{}.{key}", config::SECTION);
            git.config_set(&full_key, &value)?;
            ui.success(&format!("Set {key} = {value}"));
            Ok(())
        }

        ConfigAction::AddProtected { pattern } => {
            git.config_add(&format!("{}.protected", config::SECTION), &pattern)?;
            ui.success(&format!(
                "Added protected pattern: {}",
                console::style(pattern).cyan()
            ));
            Ok(())
        }

        ConfigAction::RemoveProtected { pattern } => {
            let key = format!("{}.protected", config::SECTION);
            let mut protected = git.config_get_all(&key)?;
            protected.retain(|p| p != &pattern);
            git.config_unset_all(&key)?;
            for p in &protected {
                git.config_add(&key, p)?;
            }
            ui.success(&format!(
                "Removed protected pattern: {}",
                console::style(pattern).cyan()
            ));
            Ok(())
        }

        ConfigAction::AddRemote { name } => {
            git.config_add(&format!("{}.remote", config::SECTION), &name)?;
            ui.success(&format!("Added remote: {}", console::style(&name).cyan()));
            Ok(())
        }

        ConfigAction::RemoveRemote { name } => {
            let key = format!("{}.remote", config::SECTION);
            let mut remotes = git.config_get_all(&key)?;
            remotes.retain(|r| r != &name);
            git.config_unset_all(&key)?;
            for r in &remotes {
                git.config_add(&key, r)?;
            }
            ui.success(&format!("Removed remote: {}", console::style(&name).cyan()));
            Ok(())
        }

        ConfigAction::Protect { branch } => {
            git.set_branch_protected(&branch, true)?;
            ui.success(&format!(
                "Branch '{}' marked as protected",
                console::style(&branch).cyan()
            ));
            Ok(())
        }

        ConfigAction::Unprotect { branch } => {
            git.set_branch_protected(&branch, false)?;
            ui.success(&format!(
                "Branch '{}' is no longer protected",
                console::style(&branch).cyan()
            ));
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
        no_pull: cli.no_pull,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{GitCommandError, GitErrorKind};

    fn git_err(kind: GitErrorKind, stderr: &str) -> anyhow::Error {
        anyhow::Error::new(GitCommandError {
            program: "git".into(),
            args: vec!["fetch".into()],
            exit_code: Some(1),
            stderr: stderr.into(),
            kind,
        })
    }

    #[test]
    fn report_error_runs_for_network_kind() {
        let ui = ui::Ui::new();
        report_error(
            &ui,
            &git_err(
                GitErrorKind::Network,
                "ssh: connect to host github.com port 22: No route to host",
            ),
        );
    }

    #[test]
    fn report_error_runs_for_auth_kind() {
        let ui = ui::Ui::new();
        report_error(
            &ui,
            &git_err(GitErrorKind::Auth, "Permission denied (publickey)."),
        );
    }

    #[test]
    fn report_error_runs_for_other_kind() {
        let ui = ui::Ui::new();
        report_error(&ui, &git_err(GitErrorKind::Other, "fatal: bad refspec"));
    }

    #[test]
    fn report_error_runs_for_plain_anyhow() {
        let ui = ui::Ui::new();
        let err = anyhow::anyhow!("top").context("inner").context("outer");
        report_error(&ui, &err);
    }

    #[test]
    fn is_cancelled_detects_interrupted_io_error() {
        let err = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::Interrupted));
        assert!(is_cancelled(&err));
    }

    #[test]
    fn is_cancelled_detects_interrupted_deep_in_chain() {
        let err = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::Interrupted))
            .context("selecting branches")
            .context("running cleanup");
        assert!(is_cancelled(&err));
    }

    #[test]
    fn is_cancelled_ignores_other_io_errors() {
        let err = anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(!is_cancelled(&err));
    }

    #[test]
    fn is_cancelled_ignores_non_io_errors() {
        let err = anyhow::anyhow!("plain error");
        assert!(!is_cancelled(&err));
        assert!(!is_cancelled(&git_err(
            GitErrorKind::Other,
            "fatal: bad refspec"
        )));
    }
}
