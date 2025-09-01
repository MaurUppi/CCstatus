use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};

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
            use chrono::{Duration, Utc};

            // Load V1 update state and check for notifications
            let state_file = crate::updater::UpdateStateFile::load();

            // Check for recent version prompts (show notification for 1 hour after prompting)
            let now = Utc::now();
            let one_hour_ago = now - Duration::hours(1);

            // Find the most recently prompted version within the last hour
            let recent_version = state_file
                .version_prompt_dates
                .iter()
                .filter(|(_, prompt_date)| **prompt_date > one_hour_ago)
                .max_by_key(|(_, prompt_date)| *prompt_date)
                .map(|(version, _)| version.clone());

            if let Some(version) = recent_version {
                return Some(SegmentData {
                    primary: format!("\u{f06b0} Update v{}!", version),
                    secondary: String::new(),
                    metadata: std::collections::HashMap::new(),
                });
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
