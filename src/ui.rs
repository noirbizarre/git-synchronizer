use console::{Style, Term};

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
        Ok(cliclack::confirm(prompt)
            .initial_value(default)
            .interact()?)
    }

    /// Present a multi-select list. Returns the selected values.
    ///
    /// `values` are the returned items; `labels` are what the user sees.
    pub fn multi_select(
        &self,
        prompt: &str,
        values: &[String],
        labels: &[String],
        defaults: &[bool],
    ) -> anyhow::Result<Vec<String>> {
        let initial_values: Vec<String> = values
            .iter()
            .zip(defaults.iter())
            .filter_map(|(val, &selected)| if selected { Some(val.clone()) } else { None })
            .collect();

        let mut ms = cliclack::multiselect(prompt);
        for (val, label) in values.iter().zip(labels.iter()) {
            ms = ms.item(val.clone(), label, "");
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

    /// Print a summary line: "1 branch deleted." or "3 branches deleted."
    pub fn summary(&self, count: usize, singular: &str, plural: &str, verb: &str) {
        let noun = if count == 1 { singular } else { plural };
        self.success(&format!("{count} {noun} {verb}."));
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
        assert!(styled.to_string().contains("test"));
    }
}
