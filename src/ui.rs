use console::{Style, Term};

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

    /// Ask for confirmation, defaulting to "no" for safety.
    pub fn confirm(&self, prompt: &str, default: bool) -> anyhow::Result<bool> {
        Ok(cliclack::confirm(prompt)
            .initial_value(default)
            .interact()?)
    }

    /// Present a multi-select list. Returns the selected values.
    ///
    /// `values` are the returned items; `labels` are what the user sees;
    /// `hints` are optional secondary text rendered next to each item
    /// (pass an empty slice to omit hints).
    pub fn multi_select(
        &self,
        prompt: &str,
        values: &[String],
        labels: &[String],
        defaults: &[bool],
        hints: &[String],
    ) -> anyhow::Result<Vec<String>> {
        let initial_values: Vec<String> = values
            .iter()
            .zip(defaults.iter())
            .filter_map(|(val, &selected)| if selected { Some(val.clone()) } else { None })
            .collect();

        let empty = String::new();
        let mut ms = cliclack::multiselect(prompt);
        for (i, (val, label)) in values.iter().zip(labels.iter()).enumerate() {
            let hint = hints.get(i).unwrap_or(&empty);
            ms = ms.item(val.clone(), label, hint.as_str());
        }
        ms = ms.initial_values(initial_values);
        ms = ms.required(false);
        Ok(ms.interact()?)
    }

    /// Ask for a text input.
    pub fn input(&self, prompt: &str, default: &str) -> anyhow::Result<String> {
        Ok(cliclack::input(prompt)
            .default_input(default)
            .required(false)
            .interact::<String>()?)
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
}
