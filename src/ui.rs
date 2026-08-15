use console::{Style, Term};
use demand::{Confirm, DemandOption, Input, MultiSelect, Spinner, SpinnerStyle};

/// Terminal handle and style presets for consistent output.
#[derive(Debug)]
pub struct Ui {
    term: Term,
    heading_style: Style,
    muted_style: Style,
    bold_style: Style,
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

impl Ui {
    pub fn new() -> Self {
        Self {
            term: Term::stderr(),
            heading_style: Style::new().cyan().bold(),
            muted_style: Style::new().dim(),
            bold_style: Style::new().bold(),
        }
    }

    // Output methods below are best-effort: I/O errors (e.g. broken pipe)
    // are silently discarded because failing to *display* a message should
    // not abort the cleanup workflow. Interactive methods (confirm,
    // multi_select, input) propagate errors because they need a response.

    /// Print a section heading.
    pub fn heading(&self, text: &str) {
        let _ = self
            .term
            .write_line(&format!("\n{}", self.heading_style.apply_to(text)));
    }

    /// Print a success message with a green checkmark prefix.
    ///
    /// The text is printed as-is (not re-colored), so callers can embed
    /// pre-styled fragments via `console::style()`.
    pub fn success(&self, text: &str) {
        let _ = self
            .term
            .write_line(&format!("{} {text}", console::style("✔").green()));
    }

    /// Print a warning with a yellow ⚠ prefix.
    ///
    /// The text is printed as-is, so callers can embed pre-styled fragments.
    pub fn warning(&self, text: &str) {
        let _ = self
            .term
            .write_line(&format!("{} {text}", console::style("⚠").yellow()));
    }

    /// Print an error with a red ✘ prefix.
    ///
    /// The text is printed as-is, so callers can embed pre-styled fragments.
    pub fn error(&self, text: &str) {
        let _ = self
            .term
            .write_line(&format!("{} {text}", console::style("✘").red()));
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
        let _ = self
            .term
            .write_line(&self.muted_style.apply_to(text).to_string());
    }

    /// Print a blank line.
    pub fn blank(&self) {
        let _ = self.term.write_line("");
    }

    /// Print a list of items with a bullet prefix.
    pub fn bullet_list(&self, items: &[String]) {
        for item in items {
            let _ = self
                .term
                .write_line(&format!("  {} {}", self.muted_style.apply_to("-"), item));
        }
    }

    /// Print an indented `label: value` pair, with the label emphasised.
    ///
    /// Owns both the indentation and the label styling so callers never need
    /// to reach for the style presets themselves.
    pub fn field(&self, label: &str, value: &str) {
        let _ = self.term.write_line(&format!(
            "  {} {}",
            self.bold_style.apply_to(format!("{label}:")),
            value
        ));
    }

    /// Ask for confirmation, pre-selecting `default`.
    pub fn confirm(&self, prompt: &str, default: bool) -> anyhow::Result<bool> {
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
        if !self.term.is_term() {
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
}
