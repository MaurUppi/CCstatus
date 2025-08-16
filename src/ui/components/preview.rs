use crate::config::Config;
// Use core module's unified exports directly
use crate::core::{collect_all_segments, StatusLineGenerator};
use ratatui::{
    layout::Rect,
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct PreviewComponent {
    preview_cache: String,
    preview_text: Text<'static>,
}

impl Default for PreviewComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewComponent {
    pub fn new() -> Self {
        Self {
            preview_cache: String::new(),
            preview_text: Text::default(),
        }
    }

    pub fn update_preview(&mut self, config: &Config) {
        self.update_preview_with_width(config, 80); // Default width
    }

    pub fn update_preview_with_width(&mut self, config: &Config, width: u16) {
        // Generate preview mock data
        let mock_input = crate::config::InputData {
            model: crate::config::Model {
                display_name: "claude-4-sonnet-20250512".to_string(),
            },
            workspace: crate::config::Workspace {
                current_dir: "/Users/haleclipse/WorkSpace/CCometixLine".to_string(),
            },
            transcript_path: "mock_preview".to_string(),
        };
        // Collect segment data
        let segments_data = collect_all_segments(config, &mock_input);
        // Generate both string and TUI text versions
        let renderer = StatusLineGenerator::new(config.clone());

        // Keep string version for compatibility (if needed elsewhere)
        self.preview_cache = renderer.generate(segments_data.clone());

        // Generate TUI-optimized text with smart segment wrapping for preview display
        // Use actual available width minus borders
        let content_width = width.saturating_sub(2);
        let preview_result = renderer.generate_for_tui_preview(segments_data, content_width);

        // Convert to owned text by cloning the spans
        let owned_lines: Vec<Line<'static>> = preview_result
            .lines
            .into_iter()
            .map(|line| {
                let owned_spans: Vec<ratatui::text::Span<'static>> = line
                    .spans
                    .into_iter()
                    .map(|span| ratatui::text::Span::styled(span.content.to_string(), span.style))
                    .collect();
                Line::from(owned_spans)
            })
            .collect();

        self.preview_text = Text::from(owned_lines);
    }

    pub fn calculate_height(&self) -> u16 {
        let line_count = self.preview_text.lines.len().max(1);
        // Min 3 (1 line + 2 borders), max 8 to prevent taking too much space
        ((line_count + 2).max(3) as u16).min(8)
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let preview = Paragraph::new(self.preview_text.clone())
            .block(Block::default().borders(Borders::ALL).title("Preview"))
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(preview, area);
    }

    pub fn get_preview_cache(&self) -> &str {
        &self.preview_cache
    }
}
