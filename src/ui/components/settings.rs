use super::segment_list::{FieldSelection, Panel};
use crate::config::{Config, SegmentId, StyleMode};
use crate::ui::utils::{ansi_color_to_description, ansi_color_to_ratatui_color};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

#[derive(Default)]
pub struct SettingsComponent;

impl SettingsComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn render(
        &self,
        f: &mut Frame,
        area: Rect,
        config: &Config,
        selected_segment: usize,
        selected_panel: &Panel,
        selected_field: &FieldSelection,
    ) {
        if let Some(segment) = config.segments.get(selected_segment) {
            let segment_name = match segment.id {
                SegmentId::Model => "Model",
                SegmentId::Directory => "Directory",
                SegmentId::Git => "Git",
                SegmentId::Usage => "Usage",
                SegmentId::Update => "Update",
            };
            let current_icon = match config.style.mode {
                StyleMode::Plain => &segment.icon.plain,
                StyleMode::NerdFont | StyleMode::Powerline => &segment.icon.nerd_font,
            };
            // Convert AnsiColor to ratatui Color
            let icon_ratatui_color = segment.colors.icon
                .as_ref()
                .map(ansi_color_to_ratatui_color)
                .unwrap_or(Color::White);
            let text_ratatui_color = segment.colors.text
                .as_ref()
                .map(ansi_color_to_ratatui_color)
                .unwrap_or(Color::White);
            let icon_color_desc = ansi_color_to_description(segment.colors.icon.as_ref());
            let text_color_desc = ansi_color_to_description(segment.colors.text.as_ref());
            let background_ratatui_color = segment.colors.background
                .as_ref()
                .map(ansi_color_to_ratatui_color)
                .unwrap_or(Color::White);
            let mut background_color_desc = ansi_color_to_description(segment.colors.background.as_ref());
            if segment.colors.background.is_none() {
                background_color_desc = "None".to_string();
            }
            let create_field_line = |field: FieldSelection, content: Vec<Span<'static>>| {
                let is_selected = *selected_panel == Panel::Settings && *selected_field == field;
                let mut spans = vec![];

                if is_selected {
                    spans.push(Span::styled(
                        "▶ ".to_string(),
                        Style::default().fg(Color::Cyan),
                    ));
                } else {
                    spans.push(Span::raw("  ".to_string()));
                }

                spans.extend(content);
                Line::from(spans)
            };
            let lines = vec![
                Line::from(format!("{} Segment", segment_name)),
                create_field_line(
                    FieldSelection::Enabled,
                    vec![Span::raw(format!(
                        "├─ Enabled: {}",
                        if segment.enabled { "✓" } else { "✗" }
                    ))],
                ),
                create_field_line(
                    FieldSelection::Icon,
                    vec![
                        Span::raw("├─ Icon: ".to_string()),
                        Span::styled(
                            current_icon.to_string(),
                            Style::default().fg(icon_ratatui_color),
                        ),
                    ],
                ),
                create_field_line(
                    FieldSelection::IconColor,
                    vec![
                        Span::raw(format!("├─ Icon Color: {} ", icon_color_desc)),
                        Span::styled("██".to_string(), Style::default().fg(icon_ratatui_color)),
                    ],
                ),
                create_field_line(
                    FieldSelection::TextColor,
                    vec![
                        Span::raw(format!("├─ Text Color: {} ", text_color_desc)),
                        Span::styled("██".to_string(), Style::default().fg(text_ratatui_color)),
                    ],
                ),
                create_field_line(
                    FieldSelection::BackgroundColor,
                    vec![
                        Span::raw(format!("├─ Background Color: {} ", background_color_desc)),
                        if segment.colors.background.is_some() {
                            Span::styled(
                                "██".to_string(),
                                Style::default().fg(background_ratatui_color),
                            )
                        } else {
                            Span::styled("--".to_string(), Style::default().fg(Color::DarkGray))
                        },
                    ],
                ),
                create_field_line(
                    FieldSelection::TextStyle,
                    vec![Span::raw(format!(
                        "├─ Text Style: Bold {}",
                        if segment.styles.text_bold {
                            "[✓]"
                        } else {
                            "[ ]"
                        }
                    ))],
                ),
                create_field_line(
                    FieldSelection::Options,
                    vec![Span::raw(format!(
                        "└─ Options: {} items",
                        segment.options.len()
                    ))],
                ),
            ];
            let text = Text::from(lines);
            let settings_block = self.create_settings_block(selected_panel);
            let settings_panel = Paragraph::new(text).block(settings_block);
            f.render_widget(settings_panel, area);
        } else {
            let settings_block = self.create_settings_block(selected_panel);
            let settings_panel = Paragraph::new("No segment selected").block(settings_block);
            f.render_widget(settings_panel, area);
        }
    }

    fn create_settings_block(&self, selected_panel: &Panel) -> Block {
        Block::default()
            .borders(Borders::ALL)
            .title("Settings")
            .border_style(if *selected_panel == Panel::Settings {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            })
    }
}
