//! Proxy Health Monitoring Module
//!
//! This module provides comprehensive proxy health checking capabilities with:
//! - Root-based and path-based URL construction strategies
//! - Tri-state health level assessment (Healthy, Degraded, Bad)  
//! - Configurable fallback and redirect following
//! - Backward compatible integration with existing NetworkMetrics

pub mod checker;
pub mod client;
pub mod config;
pub mod parsing;
pub mod url;

// Re-export public API
pub use checker::{assess_proxy_health, ProxyHealthError, ProxyHealthOutcome};
pub use client::{HealthCheckClient, HealthResponse};
pub use config::{ProxyHealthLevel, ProxyHealthOptions};
pub use parsing::{parse_health_response, validate_health_json};
pub use url::{
    build_messages_endpoint, build_path_health_url, build_root_health_url, is_official_base_url,
    normalize_base_url,
};
// ProxyHealthDetail is exported from types.rs to avoid conflicts
pub use crate::core::network::types::ProxyHealthDetail;

// Re-export client implementations conditionally
#[cfg(feature = "network-monitoring")]
pub use client::IsahcHealthCheckClient;

#[cfg(all(feature = "network-monitoring", feature = "timings-curl"))]
pub use client::CurlHealthCheckClient;

#[cfg(not(feature = "network-monitoring"))]
pub use client::MockHealthCheckClient;
