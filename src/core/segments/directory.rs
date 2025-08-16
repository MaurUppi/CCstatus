use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
use std::collections::HashMap;

#[derive(Default)]
pub struct DirectorySegment;

impl DirectorySegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for DirectorySegment {
    fn collect(&self, input: &InputData) -> Option<SegmentData> {
        let current_dir = &input.workspace.current_dir;
        let dir_name = current_dir
            .split('/')
            .next_back()
            .unwrap_or("root")
            .to_string();

        Some(SegmentData {
            primary: dir_name,
            secondary: String::new(),
            metadata: HashMap::new(),
        })
    }

    fn id(&self) -> SegmentId {
        SegmentId::Directory
    }
}
