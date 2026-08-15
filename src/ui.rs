use console::{Style, Term};
use demand::{Confirm, DemandOption, Input, MultiSelect, Spinner, SpinnerStyle};

/// Terminal handle and style presets for consistent output.
pub struct Ui {
    term: Term,
    pub heading_style: Style,
    pub muted_style: Style,
    pub bold_style: Style,
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

    /// Print muted/dim text.
    pub fn muted(&self, text: &str) {
        let _ = self
            .term
            .write_line(&self.muted_style.apply_to(text).to_string());
    }

    /// Print a plain line.
    pub fn line(&self, text: &str) {
        let _ = self.term.write_line(text);
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

    #[test]
    fn test_ui_default() {
        let ui = Ui::default();
        // Smoke test: styles should be constructable
        let styled = ui.heading_style.apply_to("test");
        assert!(styled.to_string().contains("test"));
    }

    #[test]
    fn test_success_does_not_panic() {
        let ui = Ui::new();
        ui.success("plain message");
        ui.success(&format!("with {} styling", console::style("cyan").cyan()));
    }

    #[test]
    fn test_warning_does_not_panic() {
        let ui = Ui::new();
        ui.warning("plain warning");
        ui.warning(&format!(
            "with {} styling",
            console::style("yellow").yellow()
        ));
    }

    #[test]
    fn test_error_does_not_panic() {
        let ui = Ui::new();
        ui.error("plain error");
        ui.error(&format!("with {} styling", console::style("red").red()));
    }

    #[test]
    fn test_summary_singular() {
        let ui = Ui::new();
        ui.summary(1, "branch", "branches", "deleted");
    }

    #[test]
    fn test_summary_plural() {
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
