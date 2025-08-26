//! NetworkSegment - stdin orchestration for network monitoring
//!
//! This module implements the primary orchestration component that coordinates all
//! network monitoring activities triggered by stdin events. It follows a strict
//! no-background-threads design where all monitoring is event-driven from Claude Code's
//! statusline stdin input.
//!
//! ## Architecture Overview
//!
//! NetworkSegment acts as the coordination layer between:
//! - Stdin parsing and input validation 
//! - Credential resolution via CredentialManager
//! - Error detection via JsonlMonitor  
//! - HTTP probing via HttpMonitor (single writer)
//! - Status rendering via StatusRenderer
//!
//! ## Window-Based Monitoring Strategy
//!
//! The monitoring system uses three window types with priority-based execution:
//!
//! 1. **COLD** (highest priority): Session startup probe with deduplication
//!    - Trigger: `total_duration_ms < COLD_WINDOW_MS` (default 5000ms)
//!    - Deduplication: Same session_id won't trigger multiple COLD probes
//!    - Behavior: Skip RED/GREEN windows when COLD executes
//!
//! 2. **RED** (medium priority): Error-driven rapid probing  
//!    - Trigger: `(total_duration_ms % 10_000) < 1_000` AND error detected
//!    - Frequency: Every 10 seconds (first 1 second of window)
//!    - Dependency: Requires JsonlMonitor to detect API errors first
//!
//! 3. **GREEN** (lowest priority): Regular health monitoring
//!    - Trigger: `(total_duration_ms % 300_000) < 3_000`  
//!    - Frequency: Every 300 seconds (first 3 seconds of window)
//!    - Purpose: Baseline monitoring and P95 calculation
//!
//! ## Integration Contract
//!
//! NetworkSegment follows the exact call sequence specified in the requirements:
//!
//! 1. Parse stdin → extract `total_duration_ms`, `transcript_path`, `session_id`
//! 2. `CredentialManager::get_credentials()` → `Option<ApiCredentials>`
//! 3. No credentials → `HttpMonitor::write_unknown(false)` → render → exit
//! 4. Has credentials → `JsonlMonitor::scan_tail(transcript_path)` → error detection
//! 5. Window calculation with priority: COLD > RED > GREEN
//! 6. At most one `HttpMonitor::probe()` call per stdin event
//! 7. `StatusRenderer::render_status()` → stdout

use crate::core::network::credential::CredentialManager;
use crate::core::network::debug_logger::get_debug_logger;
use crate::core::network::http_monitor::HttpMonitor;
use crate::core::network::jsonl_monitor::JsonlMonitor;
use crate::core::network::status_renderer::StatusRenderer;
use crate::core::network::types::{NetworkError, ProbeMode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::path::PathBuf;
use std::io::{self, Read};
use tokio::task;

/// Stdin input structure from Claude Code statusline
///
/// This represents the JSON payload that Claude Code sends via stdin
/// on each conversation event. Contains session metadata and timing
/// information needed for window-based monitoring decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatuslineInput {
    /// Unique session identifier for deduplication
    pub session_id: String,
    /// Path to JSONL transcript file for error detection
    pub transcript_path: String,
    /// Current working directory  
    pub cwd: String,
    /// Model information
    pub model: Value,
    /// Workspace information
    pub workspace: Value,
    /// Claude Code version
    pub version: String,
    /// Output style configuration
    pub output_style: Value,
    /// Cost and timing information
    pub cost: CostInfo,
    /// Whether session exceeds token limits
    pub exceeds_200k_tokens: bool,
}

/// Cost and timing information from Claude Code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostInfo {
    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Total duration including client processing (milliseconds)
    pub total_duration_ms: u64,
    /// API-only request duration (milliseconds)
    pub total_api_duration_ms: u64,
    /// Lines added in this session
    pub total_lines_added: u32,
    /// Lines removed in this session
    pub total_lines_removed: u32,
}

/// Window calculation results for probe decisions
#[derive(Debug, Clone, PartialEq)]
pub struct WindowDecision {
    /// Whether COLD window is active (highest priority)
    pub is_cold_window: bool,
    /// Whether RED window is active (requires error detection)
    pub is_red_window: bool,  
    /// Whether GREEN window is active (regular monitoring)
    pub is_green_window: bool,
    /// Selected probe mode based on priority and conditions
    pub probe_mode: Option<ProbeMode>,
}

/// NetworkSegment - primary orchestration component for network monitoring
///
/// Coordinates stdin-triggered monitoring workflow with window-based probe decisions.
/// Maintains strict single-writer pattern where only HttpMonitor persists state.
pub struct NetworkSegment {
    credential_manager: CredentialManager,
    jsonl_monitor: JsonlMonitor,
    http_monitor: HttpMonitor,
    status_renderer: StatusRenderer,
}

impl NetworkSegment {
    /// Create new NetworkSegment with default configuration
    ///
    /// Initializes all monitoring components with their default configurations.
    /// HttpMonitor uses the default state path (`~/.claude/ccstatus/ccstatus-monitoring.json`).
    pub fn new() -> Result<Self, NetworkError> {
        Ok(Self {
            credential_manager: CredentialManager::new()?,
            jsonl_monitor: JsonlMonitor::new(),
            http_monitor: HttpMonitor::new(None)?,
            status_renderer: StatusRenderer::new(),
        })
    }

    /// Create NetworkSegment with custom state path (for testing)
    pub fn with_state_path(state_path: PathBuf) -> Result<Self, NetworkError> {
        Ok(Self {
            credential_manager: CredentialManager::new()?,
            jsonl_monitor: JsonlMonitor::new(),
            http_monitor: HttpMonitor::new(Some(state_path))?,
            status_renderer: StatusRenderer::new(),
        })
    }

    /// Main entry point for stdin-triggered monitoring
    ///
    /// Reads JSON input from stdin and orchestrates the complete monitoring workflow
    /// according to the integration contract specified in the requirements.
    ///
    /// # Workflow
    ///
    /// 1. Parse stdin JSON input and validate required fields
    /// 2. Resolve credentials with priority: env > shell > config
    /// 3. Handle no-credentials case: write unknown status and exit
    /// 4. Scan transcript for error detection (non-COLD only)
    /// 5. Calculate window decisions with COLD > RED > GREEN priority
    /// 6. Execute at most one probe per stdin event
    /// 7. Render status to stdout for statusline display
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::InputParseError` for invalid stdin JSON.
    /// Returns `NetworkError::HomeDirNotFound` if required directories don't exist.
    /// Other errors are logged but don't prevent status rendering.
    pub async fn run_from_stdin(&mut self) -> Result<(), NetworkError> {
        let debug_logger = get_debug_logger();
        debug_logger.debug("NetworkSegment", "Starting stdin orchestration").await;

        // Step 1: Parse stdin input
        let input = self.parse_stdin_input().await?;
        
        debug_logger.debug("NetworkSegment", &format!(
            "Parsed stdin: session_id={}, total_duration_ms={}, transcript_path={}",
            input.session_id, input.cost.total_duration_ms, input.transcript_path
        )).await;

        // Step 2: Resolve credentials (env > shell > config priority)
        debug_logger.debug("NetworkSegment", "Resolving credentials...").await;
        let credentials = self.credential_manager.get_credentials().await?;
        
        if credentials.is_none() {
            debug_logger.debug("NetworkSegment", "No credentials found - writing unknown status").await;
            
            // Step 2a: Handle no credentials case
            self.http_monitor.write_unknown(false).await?;
            self.render_and_output().await?;
            return Ok(());
        }

        let creds = credentials.unwrap();
        debug_logger.debug("NetworkSegment", &format!(
            "Found credentials from: {}", creds.source
        )).await;

        // Step 3: Calculate window decisions
        let window_decision = self.calculate_window_decision(&input).await?;
        debug_logger.debug("NetworkSegment", &format!(
            "Window decision: cold={}, red={}, green={}, mode={:?}",
            window_decision.is_cold_window, window_decision.is_red_window, 
            window_decision.is_green_window, window_decision.probe_mode
        )).await;

        // Step 4: Execute probe if window is active
        if let Some(probe_mode) = window_decision.probe_mode {
            self.http_monitor.set_session_id(input.session_id.clone());
            
            let last_error = if probe_mode == ProbeMode::Red {
                // Only scan for errors in RED mode
                let (error_detected, last_error_event) = self
                    .jsonl_monitor
                    .scan_tail(&input.transcript_path)
                    .await?;
                
                debug_logger.debug("NetworkSegment", &format!(
                    "Error scan result: detected={}, error={:?}",
                    error_detected, last_error_event.as_ref().map(|e| &e.message)
                )).await;
                
                last_error_event
            } else {
                None
            };

            debug_logger.debug("NetworkSegment", &format!(
                "Executing probe: mode={:?}", probe_mode
            )).await;
            
            let outcome = self.http_monitor.probe(probe_mode, creds, last_error).await?;
            
            debug_logger.debug("NetworkSegment", &format!(
                "Probe completed: status={:?}, latency={}ms, written={}",
                outcome.status, outcome.metrics.latency_ms, outcome.state_written
            )).await;
        } else {
            debug_logger.debug("NetworkSegment", "No active window - skipping probe").await;
        }

        // Step 5: Render status to stdout
        self.render_and_output().await?;
        
        debug_logger.debug("NetworkSegment", "Stdin orchestration completed").await;
        Ok(())
    }

    /// Parse and validate stdin JSON input
    ///
    /// Reads complete stdin content and parses it as StatuslineInput JSON.
    /// Validates that required fields are present and have valid values.
    async fn parse_stdin_input(&self) -> Result<StatuslineInput, NetworkError> {
        let buffer = task::spawn_blocking(|| {
            let mut stdin = io::stdin();
            let mut buffer = Vec::new();
            stdin.read_to_end(&mut buffer).map(|_| buffer)
        }).await
        .map_err(|e| NetworkError::InputParseError(format!("Failed to join stdin task: {}", e)))?
        .map_err(|e| NetworkError::InputParseError(format!("Failed to read stdin: {}", e)))?;

        let input_str = String::from_utf8(buffer)
            .map_err(|e| NetworkError::InputParseError(format!("Invalid UTF-8 in stdin: {}", e)))?;

        let input: StatuslineInput = serde_json::from_str(&input_str)
            .map_err(|e| NetworkError::InputParseError(format!("Invalid JSON in stdin: {}", e)))?;

        // Validate required fields
        if input.session_id.is_empty() {
            return Err(NetworkError::InputParseError("session_id is required and cannot be empty".to_string()));
        }
        
        if input.transcript_path.is_empty() {
            return Err(NetworkError::InputParseError("transcript_path is required and cannot be empty".to_string()));
        }

        Ok(input)
    }

    /// Calculate window decisions based on timing and error detection
    ///
    /// Implements the window priority logic: COLD > RED > GREEN.
    /// Only one probe mode can be active per stdin event.
    ///
    /// # Window Logic
    ///
    /// - **COLD**: `total_duration_ms < COLD_WINDOW_MS` with session deduplication
    /// - **RED**: `(total_duration_ms % 10_000) < 1_000` AND error detected in transcript  
    /// - **GREEN**: `(total_duration_ms % 300_000) < 3_000`
    ///
    /// # Priority Rules
    ///
    /// 1. COLD window takes absolute priority and skips RED/GREEN evaluation
    /// 2. RED window requires both timing condition AND error detection
    /// 3. GREEN window is only evaluated if COLD and RED are not active
    pub async fn calculate_window_decision(&mut self, input: &StatuslineInput) -> Result<WindowDecision, NetworkError> {
        let total_duration_ms = input.cost.total_duration_ms;
        
        // Get COLD window threshold (default 5000ms, overrideable)  
        let cold_window_ms = env::var("ccstatus_COLD_WINDOW_MS")
            .or_else(|_| env::var("CCSTATUS_COLD_WINDOW_MS"))
            .map(|s| s.parse::<u64>().unwrap_or(5000))
            .unwrap_or(5000);

        // COLD window check (highest priority)
        let is_cold_window = total_duration_ms < cold_window_ms;
        if is_cold_window {
            // Check for session deduplication
            let should_skip_cold = self.should_skip_cold_probe(&input.session_id).await?;
            if should_skip_cold {
                return Ok(WindowDecision {
                    is_cold_window: true,
                    is_red_window: false,
                    is_green_window: false,
                    probe_mode: None, // Skip due to deduplication
                });
            }
            
            return Ok(WindowDecision {
                is_cold_window: true,
                is_red_window: false,
                is_green_window: false,
                probe_mode: Some(ProbeMode::Cold),
            });
        }

        // RED window check (medium priority) - requires error detection
        let red_timing_condition = (total_duration_ms % 10_000) < 1_000;
        if red_timing_condition {
            let (error_detected, _) = self
                .jsonl_monitor
                .scan_tail(&input.transcript_path)
                .await?;
            
            if error_detected {
                return Ok(WindowDecision {
                    is_cold_window: false,
                    is_red_window: true,
                    is_green_window: false,
                    probe_mode: Some(ProbeMode::Red),
                });
            }
        }

        // GREEN window check (lowest priority)
        let is_green_window = (total_duration_ms % 300_000) < 3_000;
        if is_green_window {
            return Ok(WindowDecision {
                is_cold_window: false,
                is_red_window: false,
                is_green_window: true,
                probe_mode: Some(ProbeMode::Green),
            });
        }

        // No active window
        Ok(WindowDecision {
            is_cold_window: false,
            is_red_window: false,
            is_green_window: false,
            probe_mode: None,
        })
    }

    /// Check if COLD probe should be skipped due to session deduplication
    ///
    /// Prevents duplicate COLD probes within the same session by checking the
    /// `last_cold_session_id` field in the monitoring state.
    async fn should_skip_cold_probe(&self, session_id: &str) -> Result<bool, NetworkError> {
        let state = self.http_monitor.load_state().await.unwrap_or_default();
        
        // Skip if the same session already triggered a COLD probe
        Ok(state.monitoring_state.last_cold_session_id.as_deref() == Some(session_id))
    }

    /// Render current status and output to stdout
    ///
    /// Loads current monitoring state and renders it using StatusRenderer.
    /// Output goes to stdout for Claude Code statusline display.
    async fn render_and_output(&self) -> Result<(), NetworkError> {
        let state = self.http_monitor.load_state().await.unwrap_or_default();
        let status_text = self.status_renderer.render_status(&state.status, &state.network);
        
        println!("{}", status_text);
        Ok(())
    }
}

impl Default for NetworkSegment {
    fn default() -> Self {
        Self::new().expect("Failed to initialize NetworkSegment")
    }
}