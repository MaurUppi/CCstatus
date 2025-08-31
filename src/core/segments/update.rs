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
            // Load V1 update state and check for notifications
            let state_file = crate::updater::UpdateStateFile::load();
            
            // Check if we have a version to notify about
            if let Some(ref last_prompted) = state_file.last_prompted_version {
                // Check if this version was prompted recently (within last hour)
                if let Some(last_check) = state_file.last_check {
                    let now = chrono::Utc::now();
                    let hours_since_check = now.signed_duration_since(last_check).num_hours();
                    
                    // Show notification for 1 hour after prompting
                    if hours_since_check < 1 {
                        return Some(SegmentData {
                            primary: format!("\u{f06b0} Update v{}!", last_prompted),
                            secondary: String::new(),
                            metadata: std::collections::HashMap::new(),
                        });
                    }
                }
            }
            
            // No notification to show
            None
        }

        #[cfg(not(feature = "self-update"))]
        None
    }

    fn id(&self) -> SegmentId {
        SegmentId::Update
    }
}
