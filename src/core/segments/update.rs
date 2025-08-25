use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
#[cfg(feature = "self-update")]
use crate::updater::UpdateState;

#[derive(Default)]
pub struct UpdateSegment;

impl UpdateSegment {
    pub fn new() -> Self {
        Self
    }
}

impl Segment for UpdateSegment {
    fn collect(&self, _input: &InputData) -> Option<SegmentData> {
        #[cfg(feature = "self-update")]
        {
            // Load update state and check for update status
            let update_state = UpdateState::load();

            update_state.status_text().map(|status_text| SegmentData {
                primary: status_text,
                secondary: String::new(),
                metadata: std::collections::HashMap::new(),
            })
        }
        
        #[cfg(not(feature = "self-update"))]
        None
    }

    fn id(&self) -> SegmentId {
        SegmentId::Update
    }
}
