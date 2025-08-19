use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Default)]
pub struct HelpComponent;

impl HelpComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        status_message: Option<&str>,
        color_picker_open: bool,
        icon_selector_open: bool,
    ) {
        let help_items = if color_picker_open {
            vec![
                "[↑↓] Navigate",
                "[Tab] Mode",
                "[Enter] Select",
                "[Esc] Cancel",
            ]
        } else if icon_selector_open {
            vec![
                "[↑↓] Navigate",
                "[Tab] Style",
                "[C] Custom",
                "[Enter] Select",
                "[Esc] Cancel",
            ]
        } else {
            vec![
                "[Tab] Switch Panel",
                "[Enter] Toggle/Edit",
                "[Shift+↑↓] Reorder",
                "[1-4] Theme",
                "[P] Switch Theme",
                "[R] Reset",
                "[E] Edit Separator",
                "[S] Save Config",
                "[W] Write Theme",
                "[Ctrl+S] Save Theme",
                "[Esc] Quit",
            ]
        };

        let status = status_message.unwrap_or("");

        // Build help text with smart wrapping - keep each shortcut as a unit
        let content_width = area.width.saturating_sub(2); // Remove borders
        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut current_width = 0usize;

        for (i, item) in help_items.iter().enumerate() {
            // Calculate item display width (character count)
            let item_width = item.chars().count();

            // Add separator for non-first items on the same line
            let needs_separator = i > 0 && !current_line.is_empty();
            let separator_width = if needs_separator { 2 } else { 0 };
            let total_width = item_width + separator_width;

            // Check if item fits on current line
            if current_width + total_width <= content_width as usize {
                // Item fits, add to current line
                if needs_separator {
                    current_line.push_str("  ");
                    current_width += 2;
                }
                current_line.push_str(item);
                current_width += item_width;
            } else {
                // Item doesn't fit, start new line
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                current_line = item.to_string();
                current_width = item_width;
            }
        }

        // Add last line if not empty
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        let mut help_text = lines.join("\n");
        if !status.is_empty() {
            help_text = format!("{}\n{}", help_text, status);
        }

        let help_paragraph = Paragraph::new(help_text)
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(help_paragraph, area);
    }
}
