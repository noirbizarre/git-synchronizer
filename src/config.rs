use anyhow::Result;

use crate::git::Git;
use crate::ui::Ui;

const SECTION: &str = "sync";

/// Stored configuration from the `[sync]` git config section.
#[derive(Debug, Clone)]
pub struct Config {
    /// Glob patterns for branches that should never be deleted.
    pub protected: Vec<String>,
    /// Remotes to consider for remote branch deletion.
    /// `None` means *all* remotes.
    pub remotes: Option<Vec<String>>,
    /// Whether to use worktrunk (wt) for worktree removal.
    /// `None` means auto-detect from worktrunk config presence.
    pub worktrunk: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            protected: vec!["main".to_string(), "master".to_string()],
            remotes: None,
            worktrunk: None,
        }
    }
}

impl Config {
    /// Load configuration from the `[sync]` git config section.
    ///
    /// Returns `None` if the section doesn't exist (first-run scenario).
    pub fn load(git: &Git) -> Result<Option<Self>> {
        if !git.config_section_exists(SECTION)? {
            return Ok(None);
        }

        let protected = git.config_get_all(&format!("{SECTION}.protected"))?;

        let remotes = {
            let vals = git.config_get_all(&format!("{SECTION}.remote"))?;
            if vals.is_empty() { None } else { Some(vals) }
        };

        let worktrunk = git
            .config_get(&format!("{SECTION}.worktrunk"))?
            .map(|v| v.eq_ignore_ascii_case("true"));

        Ok(Some(Self {
            protected,
            remotes,
            worktrunk,
        }))
    }

    /// Persist configuration to the `[sync]` git config section.
    pub fn save(&self, git: &Git) -> Result<()> {
        // Protected branches (multi-value)
        git.config_unset_all(&format!("{SECTION}.protected"))?;
        for pattern in &self.protected {
            git.config_add(&format!("{SECTION}.protected"), pattern)?;
        }

        // Remotes (multi-value, optional)
        git.config_unset_all(&format!("{SECTION}.remote"))?;
        if let Some(ref remotes) = self.remotes {
            for remote in remotes {
                git.config_add(&format!("{SECTION}.remote"), remote)?;
            }
        }

        // Worktrunk integration (optional)
        match self.worktrunk {
            Some(val) => {
                git.config_set(
                    &format!("{SECTION}.worktrunk"),
                    if val { "true" } else { "false" },
                )?;
            }
            None => {
                git.config_unset_all(&format!("{SECTION}.worktrunk"))?;
            }
        }

        Ok(())
    }

    /// Run the interactive setup wizard.
    ///
    /// Auto-detects branches and remotes, then asks the user to confirm/edit.
    pub fn interactive_setup(git: &Git, ui: &Ui) -> Result<Self> {
        ui.heading("No configuration found. Let's set up git-sync.");
        ui.blank();

        // ── Protected branches ───────────────────────────────────────

        let branches = git.local_branches()?;
        let well_known = ["main", "master", "develop", "development"];

        if branches.is_empty() {
            ui.warning("No local branches found.");
        }

        // Build selection list: branches + ability to add patterns
        let defaults: Vec<bool> = branches
            .iter()
            .map(|b| well_known.contains(&b.as_str()))
            .collect();

        let mut protected: Vec<String> = if branches.is_empty() {
            vec!["main".to_string()]
        } else {
            ui.multi_select(
                "Which branches should be protected from deletion?",
                &branches,
                &branches,
                &defaults,
            )?
        };

        let extra = ui.input(
            "Additional patterns to protect (comma-separated, e.g. release/*)",
            "",
        )?;
        for pattern in extra.split(',').map(|s| s.trim()) {
            if !pattern.is_empty() {
                protected.push(pattern.to_string());
            }
        }

        if protected.is_empty() {
            protected.push("main".to_string());
            ui.muted("  Defaulting to protecting 'main'.");
        }

        ui.blank();

        // ── Remotes ──────────────────────────────────────────────────

        let available_remotes = git.remotes()?;
        let remotes = if available_remotes.is_empty() {
            ui.muted("No remotes configured.");
            None
        } else {
            let defaults: Vec<bool> = available_remotes.iter().map(|r| r == "origin").collect();
            let selected = ui.multi_select(
                "Which remotes should merged branches be deleted from?",
                &available_remotes,
                &available_remotes,
                &defaults,
            )?;
            if selected.is_empty() {
                None
            } else {
                Some(selected)
            }
        };

        ui.blank();

        // ── Worktrunk integration ────────────────────────────────────
        let worktrunk = if crate::git::worktrunk_available() {
            ui.blank();
            let use_wt = ui.confirm(
                "Worktrunk (wt) detected. Use it for worktree removal (triggers pre/post-remove hooks)?",
                true,
            )?;
            Some(use_wt)
        } else {
            None
        };

        // ── Save ─────────────────────────────────────────────────────

        let config = Self {
            protected,
            remotes,
            worktrunk,
        };
        config.save(git)?;

        ui.success("Configuration saved to git config [sync] section.");
        ui.blank();

        Ok(config)
    }
}

/// Load config, running the interactive setup if needed.
pub fn load_or_setup(git: &Git, ui: &Ui) -> Result<Config> {
    match Config::load(git)? {
        Some(config) => Ok(config),
        None => Config::interactive_setup(git, ui),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;

    fn init_test_repo() -> (tempfile::TempDir, Git) {
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

        // Need at least one commit for branches to work
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

        let git = Git::with_workdir(false, path);
        (dir, git)
    }

    #[test]
    fn test_config_load_returns_none_when_not_configured() {
        let (_dir, git) = init_test_repo();
        let config = Config::load(&git).unwrap();
        assert!(config.is_none());
    }

    #[test]
    fn test_config_save_and_load_roundtrip() {
        let (_dir, git) = init_test_repo();

        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            remotes: Some(vec!["origin".to_string()]),
            worktrunk: None,
        };
        config.save(&git).unwrap();

        let loaded = Config::load(&git).unwrap().expect("config should exist");
        assert_eq!(loaded.protected, config.protected);
        assert_eq!(loaded.remotes, config.remotes);
        assert_eq!(loaded.worktrunk, config.worktrunk);
    }

    #[test]
    fn test_config_save_without_remotes() {
        let (_dir, git) = init_test_repo();

        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        config.save(&git).unwrap();

        let loaded = Config::load(&git).unwrap().expect("config should exist");
        assert!(loaded.remotes.is_none());
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.protected, vec!["main", "master"]);
        assert!(config.remotes.is_none());
        assert!(config.worktrunk.is_none());
    }

    #[test]
    fn test_config_save_overwrites_previous() {
        let (_dir, git) = init_test_repo();

        let config1 = Config {
            protected: vec!["main".to_string()],
            remotes: Some(vec!["origin".to_string()]),
            worktrunk: Some(true),
        };
        config1.save(&git).unwrap();

        let config2 = Config {
            protected: vec!["develop".to_string(), "release/*".to_string()],
            remotes: Some(vec!["upstream".to_string()]),
            worktrunk: Some(false),
        };
        config2.save(&git).unwrap();

        let loaded = Config::load(&git).unwrap().expect("config should exist");
        assert_eq!(loaded.protected, vec!["develop", "release/*"]);
        assert_eq!(loaded.remotes, Some(vec!["upstream".to_string()]));
        assert_eq!(loaded.worktrunk, Some(false));
    }

    #[test]
    fn test_config_worktrunk_roundtrip() {
        let (_dir, git) = init_test_repo();

        // Save with worktrunk enabled
        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: Some(true),
        };
        config.save(&git).unwrap();

        let loaded = Config::load(&git).unwrap().expect("config should exist");
        assert_eq!(loaded.worktrunk, Some(true));

        // Overwrite with worktrunk disabled
        let config2 = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: Some(false),
        };
        config2.save(&git).unwrap();

        let loaded = Config::load(&git).unwrap().expect("config should exist");
        assert_eq!(loaded.worktrunk, Some(false));

        // Overwrite with worktrunk unset
        let config3 = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        config3.save(&git).unwrap();

        let loaded = Config::load(&git).unwrap().expect("config should exist");
        assert!(loaded.worktrunk.is_none());
    }

    #[test]
    fn test_load_or_setup_returns_existing_config() {
        let (_dir, git) = init_test_repo();

        let config = Config {
            protected: vec!["main".to_string()],
            remotes: None,
            worktrunk: None,
        };
        config.save(&git).unwrap();

        // load_or_setup should return the saved config without triggering setup
        let ui = Ui::new();
        let loaded = load_or_setup(&git, &ui).unwrap();
        assert_eq!(loaded.protected, vec!["main"]);
        assert!(loaded.remotes.is_none());
    }
}
