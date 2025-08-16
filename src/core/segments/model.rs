use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

#[derive(Default)]
pub struct ModelSegment;

impl ModelSegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for ModelSegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        Some(SegmentData {
            primary: self.format_model_name(&input.model.display_name),
            secondary: String::new(),
            metadata: HashMap::new(),
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Model
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
