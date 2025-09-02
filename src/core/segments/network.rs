//! NetworkSegmentWrapper - integration bridge for network monitoring
//!
//! This module provides a wrapper around NetworkSegment that implements the Segment trait,
//! allowing network monitoring to be integrated into the statusline generation pipeline
//! while maintaining backward compatibility and feature flag isolation.

use super::{Segment, SegmentData};
use crate::config::{InputData, SegmentId};
#[cfg(feature = "network-monitoring")]
use crate::core::network::types::NetworkError;
#[cfg(feature = "network-monitoring")]
use crate::core::network::{NetworkSegment, StatuslineInput};
use std::collections::HashMap;

/// NetworkSegmentWrapper provides integration between NetworkSegment and the segment system
///
/// This wrapper handles the architectural mismatch between NetworkSegment (designed for
/// rich StatuslineInput) and the segment system (using minimal InputData). It executes
/// the complete NetworkSegment workflow and returns the rendered status as SegmentData.
#[cfg(feature = "network-monitoring")]
pub struct NetworkSegmentWrapper {
    // For now, keep it simple and just instantiate components as needed
    _placeholder: (),
}

#[cfg(feature = "network-monitoring")]
impl NetworkSegmentWrapper {
    /// Create new NetworkSegmentWrapper
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self { _placeholder: () })
    }

    /// Create NetworkSegmentWrapper with custom state path (for testing)
    pub fn with_state_path(_state_path: std::path::PathBuf) -> Result<Self, NetworkError> {
        Ok(Self { _placeholder: () })
    }

    /// Collect network monitoring data with full StatuslineInput
    ///
    /// Executes the complete NetworkSegment orchestration workflow per stdin event,
    /// then renders the resulting status for statusline display.
    pub async fn collect_with_full_input(
        &mut self,
        input: &StatuslineInput,
    ) -> Option<SegmentData> {
        // Execute orchestration workflow
        match self.run_orchestration(input).await {
            Ok(status_text) => Some(SegmentData {
                primary: status_text,
                secondary: String::new(),
                metadata: HashMap::new(),
            }),
            Err(_) => {
                // On orchestration error, fall back to existing state or unknown
                match self.get_network_status().await {
                    Ok(status_text) => Some(SegmentData {
                        primary: status_text,
                        secondary: String::new(),
                        metadata: HashMap::new(),
                    }),
                    Err(_) => Some(SegmentData {
                        primary: "⚪ Unknown".to_string(),
                        secondary: String::new(),
                        metadata: HashMap::new(),
                    }),
                }
            }
        }
    }

    /// Execute NetworkSegment orchestration workflow and return rendered status
    ///
    /// This is the core integration method that bridges the gap between the wrapper
    /// and NetworkSegment orchestration. It creates a NetworkSegment instance,
    /// runs the complete monitoring workflow, then reads and renders the result.
    async fn run_orchestration(&self, input: &StatuslineInput) -> Result<String, NetworkError> {
        use crate::core::network::debug_logger::get_debug_logger;

        let debug_logger = get_debug_logger();
        debug_logger
            .debug("NetworkWrapper", "Starting orchestration integration")
            .await;

        // Create NetworkSegment instance
        let mut segment = NetworkSegment::new()?;

        // Execute orchestration workflow with the provided input
        if let Err(e) = segment.run(input.clone()).await {
            debug_logger
                .debug("NetworkWrapper", &format!("Orchestration failed: {}", e))
                .await;
            return Err(e);
        }

        debug_logger
            .debug("NetworkWrapper", "Orchestration completed successfully")
            .await;

        // Read the updated state and render status
        self.get_network_status().await
    }

    /// Get current network monitoring status by reading existing state
    ///
    /// This reads the current monitoring state and renders it.
    /// Used as fallback when orchestration fails.
    async fn get_network_status(&self) -> Result<String, NetworkError> {
        // Create HttpMonitor and StatusRenderer to read current state
        use crate::core::network::http_monitor::HttpMonitor;
        use crate::core::network::status_renderer::StatusRenderer;

        let http_monitor = HttpMonitor::new(None)?;
        let status_renderer = StatusRenderer::new();

        let state = http_monitor.load_state().await.unwrap_or_default();
        let status_text =
            status_renderer.render_status(&state.status, &state.network, state.api_config.as_ref());
        Ok(status_text)
    }
}

#[cfg(feature = "network-monitoring")]
impl Segment for NetworkSegmentWrapper {
    /// Collect segment data (not used - special handling in collect_all_segments)
    ///
    /// This method is part of the Segment trait but won't be called directly.
    /// The network segment uses collect_with_full_input instead to receive
    /// complete StatuslineInput data rather than minimal InputData.
    fn collect(&self, _input: &InputData) -> Option<SegmentData> {
        // This method won't be called - special handling in collect_all_segments
        None
    }

    /// Return segment identifier
    fn id(&self) -> SegmentId {
        SegmentId::Network
    }
}
