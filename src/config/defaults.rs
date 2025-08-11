use super::types::{Config, SegmentsConfig};

pub const DEFAULT_CONFIG: Config = Config {
    theme: String::new(), // Set to "dark" at runtime
    segments: SegmentsConfig {
        directory: true,
        git: true,
        model: true,
        usage: true,
    },
};

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "dark".to_string(),
            segments: SegmentsConfig {
                directory: true,
                git: true,
                model: true,
                usage: true,
            },
        }
    }
}