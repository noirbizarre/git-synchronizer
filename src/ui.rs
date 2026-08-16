//! Terminal output and prompts.
//!
//! All user-facing output goes through [`Ui`] so styling, indentation and
//! error classification stay consistent. Output methods are best-effort and
//! swallow I/O errors; prompts propagate them, since they need an answer.

use console::{Style, Term};
use demand::{Confirm, DemandOption, Input, MultiSelect, Spinner, SpinnerStyle};

/// Return a display-friendly path with `$HOME` replaced by `~`.
///
/// Display only: the result is never fed back to git, which would not expand
/// the tilde. JSON reports keep the absolute path via
/// [`crate::report::path_string`].
pub fn tilde_path(abs: &std::path::Path) -> String {
    let abs = abs.to_string_lossy();
    if let Ok(home) = std::env::var("HOME")
        && let Some(rest) = abs.strip_prefix(&home)
    {
        return format!("~{rest}");
    }
    abs.into_owned()
}

/// Terminal handle and style presets for consistent output.
#[derive(Debug)]
pub struct Ui {
    term: Term,
    /// Where tabular *data* goes. See [`Ui::table_row`].
    stdout: Term,
    heading_style: Style,
    muted_style: Style,
    bold_style: Style,
    /// Suppress every output method (used by `--json`, where the machine
    /// readable document is the only output).
    quiet: bool,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    /// Create a handle writing to stderr, so output never pollutes a pipe.
    pub fn new() -> Self {
        Self {
            term: Term::stderr(),
            stdout: Term::stdout(),
            heading_style: Style::new().cyan().bold(),
            muted_style: Style::new().dim(),
            bold_style: Style::new().bold(),
            quiet: false,
        }
    }

    /// Create a handle that prints nothing.
    ///
    /// Used by `--json`, where human-readable output would be noise. Prompt
    /// methods return an error instead: JSON mode implies `--yes`, so they are
    /// unreachable, and silently answering on the user's behalf would be worse
    /// than failing loudly.
    pub fn quiet() -> Self {
        Self {
            quiet: true,
            ..Self::new()
        }
    }

    /// Write a line unless muted. Every output method funnels through this.
    fn write_line(&self, line: &str) {
        if self.quiet {
            return;
        }
        let _ = self.term.write_line(line);
    }

    /// Write a line of *data* to stdout unless muted. See [`Ui::table_row`].
    fn write_data_line(&self, line: &str) {
        if self.quiet {
            return;
        }
        let _ = self.stdout.write_line(line);
    }

    fn prompt_unavailable<T>(&self) -> anyhow::Result<T> {
        anyhow::bail!("cannot prompt in --json mode")
    }

    // Output methods below are best-effort: I/O errors (e.g. broken pipe)
    // are silently discarded because failing to *display* a message should
    // not abort the cleanup workflow. Interactive methods (confirm,
    // multi_select, input) propagate errors because they need a response.

    /// Print a section heading.
    pub fn heading(&self, text: &str) {
        self.write_line(&format!("\n{}", self.heading_style.apply_to(text)));
    }

    /// Print a success message with a green checkmark prefix.
    ///
    /// The text is printed as-is (not re-colored), so callers can embed
    /// pre-styled fragments via `console::style()`.
    pub fn success(&self, text: &str) {
        self.write_line(&format!("{} {text}", console::style("✔").green()));
    }

    /// Print a warning with a yellow ⚠ prefix.
    ///
    /// The text is printed as-is, so callers can embed pre-styled fragments.
    pub fn warning(&self, text: &str) {
        self.write_line(&format!("{} {text}", console::style("⚠").yellow()));
    }

    /// Print an error with a red ✘ prefix.
    ///
    /// The text is printed as-is, so callers can embed pre-styled fragments.
    pub fn error(&self, text: &str) {
        self.write_line(&format!("{} {text}", console::style("✘").red()));
    }

    /// Render a failed git operation with a friendly classification.
    ///
    /// Emits a single coloured line whose prefix is chosen from the
    /// [`GitErrorKind`] of the underlying [`GitCommandError`], so a network or
    /// auth failure reads as such instead of a bare "Failed to ...". Returns
    /// the detected kind so callers can decide whether to surface a follow-up
    /// warning (e.g. "detection may be stale").
    ///
    /// `action` is an infinitive phrase ("fetch from", "delete", "remove") and
    /// `target` the thing acted upon.
    pub fn report_failure(
        &self,
        action: &str,
        target: &str,
        err: &anyhow::Error,
    ) -> crate::git::GitErrorKind {
        use crate::git::{GitCommandError, GitErrorKind};

        let styled = console::style(target).red();
        if let Some(gerr) = err.downcast_ref::<GitCommandError>() {
            let cause = gerr.short_cause();
            match gerr.kind {
                GitErrorKind::Network => {
                    self.error(&format!(
                        "Network error: cannot {action} '{styled}' ({cause})."
                    ));
                }
                GitErrorKind::Auth => {
                    self.error(&format!(
                        "Authentication failed while trying to {action} '{styled}': {cause}"
                    ));
                }
                GitErrorKind::Other => {
                    self.error(&format!("Failed to {action} '{styled}': {cause}"));
                }
            }
            gerr.kind
        } else {
            self.error(&format!("Failed to {action} '{styled}': {err}"));
            GitErrorKind::Other
        }
    }

    /// Print muted/dim text.
    pub fn muted(&self, text: &str) {
        self.write_line(&self.muted_style.apply_to(text).to_string());
    }

    /// Print a blank line.
    pub fn blank(&self) {
        self.write_line("");
    }

    /// Print a list of items with a bullet prefix.
    pub fn bullet_list(&self, items: &[String]) {
        for item in items {
            self.write_line(&format!("  {} {}", self.muted_style.apply_to("-"), item));
        }
    }

    /// Print an indented, dimmed line describing an action `--dry-run`
    /// suppressed.
    ///
    /// Owns both the indentation and the `(dry-run)` marker so every preview
    /// line is formatted identically.
    pub fn dry_run(&self, text: &str) {
        self.muted(&format!("  (dry-run) {text}"));
    }

    /// Print an indented `label: value` pair, with the label emphasised.
    ///
    /// Owns both the indentation and the label styling so callers never need
    /// to reach for the style presets themselves.
    pub fn field(&self, label: &str, value: &str) {
        self.write_line(&format!(
            "  {} {}",
            self.bold_style.apply_to(format!("{label}:")),
            value
        ));
    }

    /// Print a table header row in bold.
    ///
    /// The caller owns the column padding: widths must be computed on the
    /// unstyled strings, since ANSI escapes would otherwise be counted as
    /// visible characters.
    pub fn table_header(&self, line: &str) {
        self.write_data_line(&self.bold_style.apply_to(line).to_string());
    }

    /// Print a pre-formatted, possibly pre-styled table row verbatim.
    ///
    /// Tables go to **stdout**, unlike every other output method: a listing is
    /// the answer the user asked for, not a log about producing it, and
    /// `git wipe status | grep` must work. Nothing else writes to stdout in
    /// text mode, so the stream stays a clean, greppable table; `--json` mutes
    /// this along with everything else and prints its document instead.
    pub fn table_row(&self, line: &str) {
        self.write_data_line(line);
    }

    /// Ask for confirmation, pre-selecting `default`.
    pub fn confirm(&self, prompt: &str, default: bool) -> anyhow::Result<bool> {
        if self.quiet {
            return self.prompt_unavailable();
        }
        Ok(Confirm::new(prompt).selected(default).run()?)
    }

    /// Present a multi-select list. Returns the selected values.
    ///
    /// `values` are the returned items; `labels` are what the user sees;
    /// `hints` are optional secondary text rendered next to each item
    /// (pass an empty slice to omit hints).
    /// The `a` key inverts the whole selection (select all, or deselect all
    /// when everything is already selected), and demand renders a keymap hint
    /// in the footer so the shortcut is discoverable.
    pub fn multi_select(
        &self,
        prompt: &str,
        values: &[String],
        labels: &[String],
        defaults: &[bool],
        hints: &[String],
    ) -> anyhow::Result<Vec<String>> {
        if self.quiet {
            return self.prompt_unavailable();
        }
        let mut ms = MultiSelect::new(prompt).min(0);
        for (i, val) in values.iter().enumerate() {
            let label = labels.get(i).unwrap_or(val);
            let mut option = DemandOption::new(val.clone())
                .label(label)
                .selected(defaults.get(i).copied().unwrap_or(false));
            if let Some(hint) = hints.get(i).filter(|h| !h.is_empty()) {
                option = option.description(hint);
            }
            ms = ms.option(option);
        }
        Ok(ms.run()?)
    }

    /// Ask for a text input.
    pub fn input(&self, prompt: &str, default: &str) -> anyhow::Result<String> {
        if self.quiet {
            return self.prompt_unavailable();
        }
        Ok(Input::new(prompt).default_value(default).run()?)
    }

    /// Run `op` while showing a spinner labelled `title` on stderr.
    ///
    /// Only renders on a TTY. On non-TTY stderr (pipes, CI, tests) it runs
    /// `op` directly — no background thread and no Ctrl-C handler installed.
    /// On a TTY, demand's spinner installs its own Ctrl-C handler that exits
    /// with code 130; a mid-spinner interrupt therefore does not print
    /// "Cancelled." (by design — 130 is the conventional SIGINT exit code).
    ///
    /// The spinner clears its own line before returning, so callers must
    /// print any success/error line *after* this returns, never inside `op`.
    pub fn spinner<T, F>(&self, title: &str, op: F) -> T
    where
        F: FnOnce() -> T + Send,
        T: Send,
    {
        if self.quiet || !self.term.is_term() {
            return op();
        }
        Spinner::new(title.to_string())
            .style(&SpinnerStyle::minidots())
            .run(|_action| op())
            .expect("spinner render failed on a TTY")
    }

    /// Print a summary line: "✔ 1 branch deleted." or "✔ 3 branches deleted."
    ///
    /// The count and noun are styled in cyan; the verb and period use the
    /// terminal's default colour.
    pub fn summary(&self, count: usize, singular: &str, plural: &str, verb: &str) {
        let noun = if count == 1 { singular } else { plural };
        self.success(&format!(
            "{} {verb}.",
            console::style(format!("{count} {noun}")).cyan(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{GitCommandError, GitErrorKind};

    fn git_err(kind: GitErrorKind, stderr: &str) -> anyhow::Error {
        anyhow::Error::new(GitCommandError {
            program: "git".into(),
            args: vec!["fetch".into(), "--prune".into(), "origin".into()],
            exit_code: Some(1),
            stderr: stderr.into(),
            kind,
        })
    }

    #[test]
    fn report_failure_classifies_network() {
        let ui = Ui::new();
        let err = git_err(
            GitErrorKind::Network,
            "ssh: connect to host github.com port 22: No route to host",
        );
        assert_eq!(
            ui.report_failure("fetch from", "origin", &err),
            GitErrorKind::Network
        );
    }

    #[test]
    fn report_failure_classifies_auth() {
        let ui = Ui::new();
        let err = git_err(GitErrorKind::Auth, "Permission denied (publickey).");
        assert_eq!(
            ui.report_failure("fetch from", "origin", &err),
            GitErrorKind::Auth
        );
    }

    #[test]
    fn report_failure_classifies_other() {
        let ui = Ui::new();
        let err = git_err(GitErrorKind::Other, "fatal: refusing to fetch");
        assert_eq!(
            ui.report_failure("fetch from", "origin", &err),
            GitErrorKind::Other
        );
    }

    #[test]
    fn report_failure_handles_non_git_error() {
        let ui = Ui::new();
        let err = anyhow::anyhow!("something went wrong");
        assert_eq!(
            ui.report_failure("pull", "feature", &err),
            GitErrorKind::Other
        );
    }

    #[test]
    fn ui_default() {
        let ui = Ui::default();
        // Smoke test: styles should be constructable
        let styled = ui.heading_style.apply_to("test");
        assert!(styled.to_string().contains("test"));
    }

    #[test]
    fn success_does_not_panic() {
        let ui = Ui::new();
        ui.success("plain message");
        ui.success(&format!("with {} styling", console::style("cyan").cyan()));
    }

    #[test]
    fn warning_does_not_panic() {
        let ui = Ui::new();
        ui.warning("plain warning");
        ui.warning(&format!(
            "with {} styling",
            console::style("yellow").yellow()
        ));
    }

    #[test]
    fn error_does_not_panic() {
        let ui = Ui::new();
        ui.error("plain error");
        ui.error(&format!("with {} styling", console::style("red").red()));
    }

    #[test]
    fn summary_singular() {
        let ui = Ui::new();
        ui.summary(1, "branch", "branches", "deleted");
    }

    #[test]
    fn summary_plural() {
        let ui = Ui::new();
        ui.summary(5, "branch", "branches", "deleted");
    }

    #[test]
    fn spinner_is_noop_and_returns_value_when_not_a_tty() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let ui = Ui::new(); // stderr not a TTY under tests
        let ran = AtomicBool::new(false);
        let out: i32 = ui.spinner("work", || {
            ran.store(true, Ordering::SeqCst);
            42
        });
        assert!(ran.load(Ordering::SeqCst));
        assert_eq!(out, 42);
    }

    #[test]
    fn quiet_output_methods_do_not_panic() {
        let ui = Ui::quiet();
        ui.heading("heading");
        ui.success("success");
        ui.warning("warning");
        ui.error("error");
        ui.muted("muted");
        ui.blank();
        ui.bullet_list(&["one".to_string()]);
        ui.dry_run("would do something");
        ui.field("label", "value");
        ui.summary(2, "branch", "branches", "deleted");
        assert_eq!(
            ui.report_failure("pull", "feature", &anyhow::anyhow!("boom")),
            GitErrorKind::Other
        );
    }

    #[test]
    fn quiet_spinner_still_runs_the_operation() {
        let ui = Ui::quiet();
        assert_eq!(ui.spinner("work", || 7), 7);
    }

    #[test]
    fn quiet_prompts_fail() {
        let ui = Ui::quiet();
        assert!(ui.confirm("ok?", true).is_err());
        assert!(ui.input("name", "default").is_err());
        assert!(
            ui.multi_select("pick", &["a".to_string()], &[], &[], &[])
                .is_err()
        );
    }

    #[test]
    #[cfg(unix)]
    fn tilde_path_replaces_home() {
        let home = std::env::var("HOME").expect("HOME must be set for this test");
        assert_eq!(
            tilde_path(&std::path::PathBuf::from(format!("{home}/projects/repo"))),
            "~/projects/repo"
        );
    }

    #[test]
    #[cfg(unix)]
    fn tilde_path_preserves_non_home_path() {
        assert_eq!(
            tilde_path(std::path::Path::new("/tmp/some/path")),
            "/tmp/some/path"
        );
    }

    #[test]
    #[cfg(unix)]
    fn tilde_path_exact_home() {
        let home = std::env::var("HOME").expect("HOME must be set for this test");
        // Exact HOME path (no trailing slash) should become just "~"
        assert_eq!(tilde_path(std::path::Path::new(&home)), "~");
    }

    #[test]
    fn table_helpers_do_not_panic() {
        let ui = Ui::new();
        ui.table_header("AGE  STATUS  BRANCH  PATH");
        ui.table_row("3d   merged  feature ~/repo");
        let quiet = Ui::quiet();
        quiet.table_header("AGE");
        quiet.table_row("3d");
    }
}
