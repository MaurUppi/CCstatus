use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub theme: String,
    pub segments: SegmentsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SegmentsConfig {
    pub directory: bool,
    pub git: bool,
    pub model: bool,
    pub usage: bool,
}

// Data structures compatible with existing main.rs
#[derive(Deserialize)]
pub struct Model {
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct Workspace {
    pub current_dir: String,
}

#[derive(Deserialize)]
pub struct InputData {
    pub model: Model,
    pub workspace: Workspace,
    pub transcript_path: String,
}

#[derive(Deserialize)]
pub struct Usage {
    // Claude-style fields
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
    // OpenAI-style fields with aliases for compatibility
    #[serde(alias = "prompt_tokens")]
    pub prompt_tokens: Option<u32>,
    #[serde(alias = "completion_tokens")]
    pub completion_tokens: Option<u32>,
    #[serde(alias = "total_tokens")]
    pub total_tokens: Option<u32>,
}

#[derive(Deserialize)]
pub struct Message {
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub struct TranscriptEntry {
    pub r#type: Option<String>,
    pub message: Option<Message>,
}
