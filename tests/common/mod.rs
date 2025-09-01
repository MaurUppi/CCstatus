//! Common test utilities and helpers for network monitoring tests

use std::env;
use tempfile::TempDir;

/// Test helper to create a temporary directory for test files
pub fn create_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

/// Test helper to setup isolated environment for credential tests
pub struct IsolatedEnv {
    original_base_url: Option<String>,
    original_bedrock_base_url: Option<String>,
    original_vertex_base_url: Option<String>,
    original_auth_token: Option<String>,
    original_api_key: Option<String>,
    original_home: Option<String>,
}

impl IsolatedEnv {
    pub fn new() -> Self {
        let original_base_url = env::var("ANTHROPIC_BASE_URL").ok();
        let original_bedrock_base_url = env::var("ANTHROPIC_BEDROCK_BASE_URL").ok();
        let original_vertex_base_url = env::var("ANTHROPIC_VERTEX_BASE_URL").ok();
        let original_auth_token = env::var("ANTHROPIC_AUTH_TOKEN").ok();
        let original_api_key = env::var("ANTHROPIC_API_KEY").ok();
        let original_home = env::var("HOME").ok();

        // Clear all credential-related environment variables
        env::remove_var("ANTHROPIC_BASE_URL");
        env::remove_var("ANTHROPIC_BEDROCK_BASE_URL");
        env::remove_var("ANTHROPIC_VERTEX_BASE_URL");
        env::remove_var("ANTHROPIC_AUTH_TOKEN");
        env::remove_var("ANTHROPIC_API_KEY");

        Self {
            original_base_url,
            original_bedrock_base_url,
            original_vertex_base_url,
            original_auth_token,
            original_api_key,
            original_home,
        }
    }

    pub fn set_temp_home(&self, temp_dir: &std::path::Path) {
        env::set_var("HOME", temp_dir);
    }

    pub fn set_test_credentials(&self, base_url: &str, token: &str) {
        env::set_var("ANTHROPIC_BASE_URL", base_url);
        env::set_var("ANTHROPIC_AUTH_TOKEN", token);
    }
}

impl Drop for IsolatedEnv {
    fn drop(&mut self) {
        // Restore all original environment variables
        if let Some(url) = &self.original_base_url {
            env::set_var("ANTHROPIC_BASE_URL", url);
        } else {
            env::remove_var("ANTHROPIC_BASE_URL");
        }

        if let Some(url) = &self.original_bedrock_base_url {
            env::set_var("ANTHROPIC_BEDROCK_BASE_URL", url);
        } else {
            env::remove_var("ANTHROPIC_BEDROCK_BASE_URL");
        }

        if let Some(url) = &self.original_vertex_base_url {
            env::set_var("ANTHROPIC_VERTEX_BASE_URL", url);
        } else {
            env::remove_var("ANTHROPIC_VERTEX_BASE_URL");
        }

        if let Some(token) = &self.original_auth_token {
            env::set_var("ANTHROPIC_AUTH_TOKEN", token);
        } else {
            env::remove_var("ANTHROPIC_AUTH_TOKEN");
        }

        if let Some(key) = &self.original_api_key {
            env::set_var("ANTHROPIC_API_KEY", key);
        } else {
            env::remove_var("ANTHROPIC_API_KEY");
        }

        if let Some(home) = &self.original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    }
}

/// Create test InputData for segment tests
pub fn create_test_input_data() -> ccstatus::config::InputData {
    ccstatus::config::InputData {
        model: ccstatus::config::types::Model {
            display_name: "test-model".to_string(),
        },
        workspace: ccstatus::config::types::Workspace {
            current_dir: "/test".to_string(),
        },
        transcript_path: "/test/transcript.json".to_string(),
    }
}
