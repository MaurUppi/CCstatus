use crate::config::InputData;
use crate::core::segments::Segment;
use crate::updater::UpdateState;

/// Update notification segment
pub struct UpdateSegment {
    state: UpdateState,
}

impl UpdateSegment {
    pub fn new() -> Self {
        Self {
            state: UpdateState::load(),
        }
    }
}

impl Default for UpdateSegment {
    fn default() -> Self {
        Self::new()
    }
}

impl Segment for UpdateSegment {
    fn render(&self, _input: &InputData) -> String {
        self.state.status_text().unwrap_or_default()
    }

    fn enabled(&self) -> bool {
        self.state.status_text().is_some()
    }
}
