//! Proxy Health Monitoring Module
//!
//! This module provides comprehensive proxy health checking capabilities with:
//! - Root-based and path-based URL construction strategies
//! - Tri-state health level assessment (Healthy, Degraded, Bad)  
//! - Configurable fallback and redirect following
//! - Backward compatible integration with existing NetworkMetrics

pub mod client;
pub mod config;
pub mod url;
pub mod parsing;
pub mod checker;



// Re-export public API
pub use client::{HealthCheckClient, HealthResponse};
pub use config::{ProxyHealthOptions, ProxyHealthLevel};
pub use url::{build_root_health_url, build_path_health_url, normalize_base_url, is_official_base_url, build_messages_endpoint};
pub use parsing::{parse_health_response, validate_health_json};
pub use checker::{assess_proxy_health, ProxyHealthOutcome, ProxyHealthError};
// ProxyHealthDetail is exported from types.rs to avoid conflicts
pub use crate::core::network::types::ProxyHealthDetail;

// Re-export client implementations conditionally
#[cfg(feature = "network-monitoring")]
pub use client::IsahcHealthCheckClient;

#[cfg(not(feature = "network-monitoring"))]
pub use client::MockHealthCheckClient;