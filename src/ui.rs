use console::{Style, Term};
use dialoguer::{Confirm, Input, MultiSelect, Select};

/// Terminal handle and style presets for consistent output.
pub struct Ui {
    term: Term,
    pub heading: Style,
    pub success: Style,
    pub warning: Style,
    pub muted: Style,
    pub bold: Style,
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
            heading: Style::new().cyan().bold(),
            success: Style::new().green(),
            warning: Style::new().yellow(),
            muted: Style::new().dim(),
            bold: Style::new().bold(),
        }
    }

    /// Print a section heading.
    pub fn heading(&self, text: &str) {
        let _ = self
            .term
            .write_line(&format!("\n{}", self.heading.apply_to(text)));
    }

    /// Print a success message.
    pub fn success(&self, text: &str) {
        let _ = self
            .term
            .write_line(&self.success.apply_to(text).to_string());
    }

    /// Print a warning.
    pub fn warning(&self, text: &str) {
        let _ = self
            .term
            .write_line(&self.warning.apply_to(text).to_string());
    }

    /// Print muted/dim text.
    pub fn muted(&self, text: &str) {
        let _ = self.term.write_line(&self.muted.apply_to(text).to_string());
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
                .write_line(&format!("  {} {}", self.muted.apply_to("-"), item));
        }
    }

    /// Ask for confirmation, defaulting to "no" for safety.
    pub fn confirm(&self, prompt: &str, default: bool) -> anyhow::Result<bool> {
        Ok(Confirm::new()
            .with_prompt(prompt)
            .default(default)
            .interact()?)
    }

    /// Present a multi-select list. Returns indices of selected items.
    pub fn multi_select(
        &self,
        prompt: &str,
        items: &[String],
        defaults: &[bool],
    ) -> anyhow::Result<Vec<usize>> {
        Ok(MultiSelect::new()
            .with_prompt(prompt)
            .items(items)
            .defaults(defaults)
            .interact()?)
    }

    /// Present a single-select list. Returns the index of the selected item.
    pub fn select(&self, prompt: &str, items: &[String]) -> anyhow::Result<usize> {
        Ok(Select::new()
            .with_prompt(prompt)
            .items(items)
            .default(0)
            .interact()?)
    }

    /// Ask for a text input.
    pub fn input(&self, prompt: &str, default: &str) -> anyhow::Result<String> {
        Ok(Input::new()
            .with_prompt(prompt)
            .default(default.to_string())
            .allow_empty(true)
            .interact_text()?)
    }

    /// Print a summary line: "N branch(es) deleted."
    pub fn summary(&self, count: usize, noun: &str, verb: &str) {
        let plural = if count == 1 { "" } else { "es" };
        self.success(&format!("{count} {noun}{plural} {verb}."));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_default() {
        let ui = Ui::default();
        // Smoke test: styles should be constructable
        let styled = ui.heading.apply_to("test");
        assert_eq!(styled.to_string().contains("test"), true);
    }
}
