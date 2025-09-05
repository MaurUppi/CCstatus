pub mod credential;
pub mod debug_logger;
pub mod error_tracker;
pub mod http_monitor;
pub mod jsonl_monitor;
pub mod network_segment;
pub mod oauth_masquerade;
pub mod proxy_health;
pub mod status_renderer;
pub mod types;

// Re-export commonly used items
pub use credential::CredentialManager;
pub use debug_logger::{get_debug_logger, EnhancedDebugLogger, JsonlLoggerConfig};
pub use http_monitor::{ClockTrait, HttpClientTrait, HttpMonitor};
pub use jsonl_monitor::JsonlMonitor;
pub use network_segment::{CostInfo, NetworkSegment, StatuslineInput, WindowDecision};
pub use oauth_masquerade::{run_probe, OauthMasqueradeOptions, OauthMasqueradeResult};
pub use status_renderer::StatusRenderer;
pub use types::*;
