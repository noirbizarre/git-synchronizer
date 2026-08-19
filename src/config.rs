//! The `[wipe]` git config section, and the first-run setup wizard.
//!
//! Configuration is read from any git config scope but always written to the
//! repository-local `.git/config`. [`Config::try_load`] returns `None` when the
//! section is absent, which is what triggers [`run_setup_wizard`].

use anyhow::{Context, Result};

use crate::branches::Effort;
use crate::duration::MinAge;
use crate::git::Git;
use crate::ui::Ui;

/// The git config section name used for all git-wipe settings.
pub const SECTION: &str = "wipe";

/// Split a comma-separated prompt answer into trimmed, non-empty patterns.
fn parse_patterns(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parse a `wipe.jobs` value.
///
/// Zero is rejected rather than silently promoted to one: it is far more likely
/// to be a mistake than a request, and `--jobs 0` is refused by clap for the
/// same reason.
fn parse_jobs(input: &str) -> Result<u32> {
    let jobs: u32 = input
        .trim()
        .parse()
        .with_context(|| format!("'{input}' is not a number"))?;
    if jobs == 0 {
        anyhow::bail!("must be at least 1");
    }
    Ok(jobs)
}

/// Stored configuration from the `[wipe]` git config section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Glob patterns for branches that should never be deleted.
    pub protected: Vec<String>,
    /// Glob patterns for branches git-wipe should ignore entirely: they are not
    /// fetched, never become merge targets, and never appear as candidates.
    pub ignore: Vec<String>,
    /// Remotes to consider for remote branch deletion.
    /// `None` means *all* remotes.
    pub remotes: Option<Vec<String>>,
    /// Whether to use worktrunk (wt) for worktree removal.
    /// `None` means auto-detect from worktrunk config presence.
    pub worktrunk: Option<bool>,
    /// How thorough merge detection should be.
    /// `None` means use [`Effort::default`].
    pub effort: Option<Effort>,
    /// Minimum age a worktree must have before it may be removed.
    /// `None` means use [`MinAge::default`], i.e. no guard.
    pub min_age: Option<MinAge>,
    /// How many read-only git probes may run at once during analysis.
    /// `None` means use the CPU count.
    pub jobs: Option<u32>,
}

/// A conventional starting point, **not** the value git-wipe falls back to at
/// runtime.
///
/// Production never reaches this: [`Config::try_load`] either returns the
/// stored configuration or `None`, and `None` runs the setup wizard, whose own
/// fallback is `main` alone. The extra `master` here exists so tests and
/// external callers get a sensible two-branch default.
impl Default for Config {
    fn default() -> Self {
        Self {
            protected: vec!["main".to_string(), "master".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        }
    }
}

impl Config {
    /// Load configuration from the `[wipe]` git config section.
    ///
    /// Returns `Ok(None)` if the section doesn't exist (first-run scenario),
    /// which is why this is `try_load` rather than `load`: absence is an
    /// expected outcome, distinct from a failure to read the config.
    pub fn try_load(git: &Git) -> Result<Option<Self>> {
        if !git.config_section_exists(SECTION)? {
            return Ok(None);
        }

        let protected = git.config_get_all(&format!("{SECTION}.protected"))?;
        let ignore = git.config_get_all(&format!("{SECTION}.ignore"))?;

        let remotes = {
            let vals = git.config_get_all(&format!("{SECTION}.remote"))?;
            if vals.is_empty() { None } else { Some(vals) }
        };

        let worktrunk = git
            .config_get(&format!("{SECTION}.worktrunk"))?
            .map(|v| v.eq_ignore_ascii_case("true"));

        let effort = git
            .config_get(&format!("{SECTION}.effort"))?
            .map(|v| v.parse::<Effort>())
            .transpose()
            .with_context(|| format!("invalid {SECTION}.effort in git config"))?;

        let min_age = git
            .config_get(&format!("{SECTION}.minage"))?
            .map(|v| v.parse::<MinAge>())
            .transpose()
            .with_context(|| format!("invalid {SECTION}.minage in git config"))?;

        let jobs = git
            .config_get(&format!("{SECTION}.jobs"))?
            .map(|v| parse_jobs(&v))
            .transpose()
            .with_context(|| format!("invalid {SECTION}.jobs in git config"))?;

        Ok(Some(Self {
            protected,
            ignore,
            remotes,
            worktrunk,
            effort,
            min_age,
            jobs,
        }))
    }

    /// Persist configuration to the `[wipe]` git config section.
    pub fn save(&self, git: &Git) -> Result<()> {
        // Protected branches (multi-value)
        git.config_unset_all(&format!("{SECTION}.protected"))?;
        for pattern in &self.protected {
            git.config_add(&format!("{SECTION}.protected"), pattern)?;
        }

        // Ignored branch patterns (multi-value)
        git.config_unset_all(&format!("{SECTION}.ignore"))?;
        for pattern in &self.ignore {
            git.config_add(&format!("{SECTION}.ignore"), pattern)?;
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

        // Merge detection effort level (optional)
        match self.effort {
            Some(effort) => {
                git.config_set(&format!("{SECTION}.effort"), &effort.as_u8().to_string())?;
            }
            None => {
                git.config_unset_all(&format!("{SECTION}.effort"))?;
            }
        }

        // Minimum worktree age (optional)
        match self.min_age {
            Some(min_age) => {
                git.config_set(&format!("{SECTION}.minage"), &min_age.to_string())?;
            }
            None => {
                git.config_unset_all(&format!("{SECTION}.minage"))?;
            }
        }

        // Analysis parallelism (optional)
        match self.jobs {
            Some(jobs) => {
                git.config_set(&format!("{SECTION}.jobs"), &jobs.to_string())?;
            }
            None => {
                git.config_unset_all(&format!("{SECTION}.jobs"))?;
            }
        }

        Ok(())
    }

    /// Run the interactive setup wizard.
    ///
    /// Auto-detects branches and remotes, then asks the user to confirm/edit.
    pub fn interactive_setup(git: &Git, ui: &Ui) -> Result<Self> {
        ui.heading("No configuration found. Let's set up git-wipe.");
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
                &[],
                true,
            )?
        };

        let extra = ui.input(
            "Additional patterns to protect (comma-separated, e.g. release/*)",
            "",
        )?;
        protected.extend(parse_patterns(&extra));

        if protected.is_empty() {
            protected.push("main".to_string());
            ui.muted("  Defaulting to protecting 'main'.");
        }

        ui.blank();

        // ── Ignored branches ─────────────────────────────────────────

        let ignore_input = ui.input(
            "Branch patterns to ignore entirely (comma-separated, e.g. wip/*)",
            "",
        )?;
        let ignore = parse_patterns(&ignore_input);

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
                &[],
                false,
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

        // Effort and min age are deliberately not asked here: they are
        // power-user knobs with sensible defaults, set later with
        // `git wipe config set effort <n>` / `... set minage <duration>`.
        let config = Self {
            protected,
            ignore,
            remotes,
            worktrunk,
            effort: None,
            min_age: None,
            jobs: None,
        };
        config.save(git)?;

        ui.success("Configuration saved to git config [wipe] section.");
        ui.blank();

        Ok(config)
    }
}

/// Load config, running the interactive setup if needed.
pub fn load_or_setup(git: &Git, ui: &Ui) -> Result<Config> {
    match Config::try_load(git)? {
        Some(config) => Ok(config),
        None => Config::interactive_setup(git, ui),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_load_returns_none_when_not_configured() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;
        let config = Config::try_load(&git)?;
        assert!(config.is_none());
        Ok(())
    }

    #[test]
    fn config_save_and_load_roundtrip() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config = Config {
            protected: vec!["main".to_string(), "release/*".to_string()],
            ignore: Vec::new(),
            remotes: Some(vec!["origin".to_string()]),
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        };
        config.save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert_eq!(loaded.protected, config.protected);
        assert_eq!(loaded.remotes, config.remotes);
        assert_eq!(loaded.worktrunk, config.worktrunk);
        Ok(())
    }

    #[test]
    fn config_save_without_remotes() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        };
        config.save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert!(loaded.remotes.is_none());
        Ok(())
    }

    #[test]
    fn config_default() {
        let config = Config::default();
        assert_eq!(config.protected, vec!["main", "master"]);
        assert!(config.ignore.is_empty());
        assert!(config.remotes.is_none());
        assert!(config.worktrunk.is_none());
    }

    #[test]
    fn parse_patterns_splits_trims_and_drops_empties() {
        assert!(parse_patterns("").is_empty());
        assert!(parse_patterns("   ").is_empty());
        assert!(parse_patterns(",,").is_empty());
        assert_eq!(parse_patterns("wip/*"), vec!["wip/*".to_string()]);
        assert_eq!(
            parse_patterns(" wip/* , scratch ,, tmp"),
            vec![
                "wip/*".to_string(),
                "scratch".to_string(),
                "tmp".to_string()
            ]
        );
    }

    #[test]
    fn config_ignore_roundtrip() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config = Config {
            protected: vec!["main".to_string()],
            ignore: vec!["wip/*".to_string(), "scratch".to_string()],
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        };
        config.save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert_eq!(loaded.ignore, config.ignore);
        Ok(())
    }

    #[test]
    fn config_ignore_defaults_to_empty_when_key_absent() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        }
        .save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert!(loaded.ignore.is_empty());
        Ok(())
    }

    #[test]
    fn config_save_clears_removed_ignore_patterns() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        Config {
            protected: vec!["main".to_string()],
            ignore: vec!["wip/*".to_string()],
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        }
        .save(&git)?;

        Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        }
        .save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert!(loaded.ignore.is_empty());
        Ok(())
    }

    #[test]
    fn config_save_overwrites_previous() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config1 = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: Some(vec!["origin".to_string()]),
            worktrunk: Some(true),
            effort: None,
            min_age: None,
            jobs: None,
        };
        config1.save(&git)?;

        let config2 = Config {
            protected: vec!["develop".to_string(), "release/*".to_string()],
            ignore: Vec::new(),
            remotes: Some(vec!["upstream".to_string()]),
            worktrunk: Some(false),
            effort: None,
            min_age: None,
            jobs: None,
        };
        config2.save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert_eq!(loaded.protected, vec!["develop", "release/*"]);
        assert_eq!(loaded.remotes, Some(vec!["upstream".to_string()]));
        assert_eq!(loaded.worktrunk, Some(false));
        Ok(())
    }

    #[test]
    fn config_effort_roundtrip() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config = Config {
            effort: Some(Effort::Thorough),
            ..Config::default()
        };
        config.save(&git)?;
        assert_eq!(
            Config::try_load(&git)?.expect("config should exist").effort,
            Some(Effort::Thorough)
        );

        // Unsetting it removes the key entirely, falling back to the default.
        let config = Config {
            effort: None,
            ..Config::default()
        };
        config.save(&git)?;
        assert!(
            Config::try_load(&git)?
                .expect("config should exist")
                .effort
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn config_effort_rejects_an_invalid_stored_value() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;
        Config::default().save(&git)?;
        git.config_set(&format!("{SECTION}.effort"), "9")?;

        let err = Config::try_load(&git).expect_err("invalid effort must fail to load");
        assert!(
            format!("{err:#}").contains("wipe.effort"),
            "error should name the offending key, got: {err:#}"
        );
        Ok(())
    }

    #[test]
    fn config_jobs_roundtrips_and_rejects_invalid_stored_values() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config = Config {
            jobs: Some(4),
            ..Config::default()
        };
        config.save(&git)?;
        assert_eq!(Config::try_load(&git)?.expect("saved").jobs, Some(4));

        // Unsetting removes the key rather than storing a sentinel.
        Config::default().save(&git)?;
        assert!(Config::try_load(&git)?.expect("saved").jobs.is_none());

        // Zero is a mistake, not a request for "auto".
        git.config_set(&format!("{SECTION}.jobs"), "0")?;
        let err = Config::try_load(&git).expect_err("jobs = 0 must fail to load");
        assert!(
            format!("{err:#}").contains("wipe.jobs"),
            "error should name the offending key, got: {err:#}"
        );

        git.config_set(&format!("{SECTION}.jobs"), "lots")?;
        assert!(Config::try_load(&git).is_err());
        Ok(())
    }

    #[test]
    fn config_worktrunk_roundtrip() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        // Save with worktrunk enabled
        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: Some(true),
            effort: None,
            min_age: None,
            jobs: None,
        };
        config.save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert_eq!(loaded.worktrunk, Some(true));

        // Overwrite with worktrunk disabled
        let config2 = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: Some(false),
            effort: None,
            min_age: None,
            jobs: None,
        };
        config2.save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert_eq!(loaded.worktrunk, Some(false));

        // Overwrite with worktrunk unset
        let config3 = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        };
        config3.save(&git)?;

        let loaded = Config::try_load(&git)?.expect("config should exist");
        assert!(loaded.worktrunk.is_none());
        Ok(())
    }

    #[test]
    fn load_or_setup_returns_existing_config() -> Result<()> {
        let (_dir, git) = crate::test_helpers::init_repo()?;

        let config = Config {
            protected: vec!["main".to_string()],
            ignore: Vec::new(),
            remotes: None,
            worktrunk: None,
            effort: None,
            min_age: None,
            jobs: None,
        };
        config.save(&git)?;

        // load_or_setup should return the saved config without triggering setup
        let ui = Ui::new();
        let loaded = load_or_setup(&git, &ui)?;
        assert_eq!(loaded.protected, vec!["main"]);
        assert!(loaded.remotes.is_none());
        Ok(())
    }
}
