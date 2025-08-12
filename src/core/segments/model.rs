use super::Segment;
use crate::config::InputData;

pub struct ModelSegment {
    enabled: bool,
}

impl ModelSegment {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl Segment for ModelSegment {
    fn render(&self, input: &InputData) -> String {
        if !self.enabled {
            return String::new();
        }

        format!(
            "\u{e26d} {}",
            self.format_model_name(&input.model.display_name)
        )
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

impl ModelSegment {
    fn format_model_name(&self, display_name: &str) -> String {
        // Simplify model display names
        match display_name {
            name if name.contains("claude-3-5-sonnet") => "Sonnet 3.5".to_string(),
            name if name.contains("claude-3-7-sonnet") => "Sonnet 3.7".to_string(),
            name if name.contains("claude-3-sonnet") => "Sonnet 3".to_string(),
            name if name.contains("claude-3-haiku") => "Haiku 3".to_string(),
            name if name.contains("claude-4-sonnet") => "Sonnet 4".to_string(),
            name if name.contains("claude-4-opus") => "Opus 4".to_string(),
            name if name.contains("sonnet-4") => "Sonnet 4".to_string(),
            _ => display_name.to_string(),
        }
    }
}
