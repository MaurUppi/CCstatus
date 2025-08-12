use super::Segment;
use crate::config::InputData;
use std::path::Path;

pub struct DirectorySegment {
    enabled: bool,
}

impl DirectorySegment {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl Segment for DirectorySegment {
    fn render(&self, input: &InputData) -> String {
        if !self.enabled {
            return String::new();
        }

        let dir_name = get_current_dir_name(&input.workspace.current_dir);
        format!("\u{f024b} {}", dir_name)
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}

fn get_current_dir_name<P: AsRef<Path>>(path: P) -> String {
    path.as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}
