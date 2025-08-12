use super::types::Config;
use std::path::Path;

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load() -> Config {
        // Return default config for now, implement multi-layer loading later
        Config::default()
    }

    pub fn load_from_path<P: AsRef<Path>>(_path: P) -> Result<Config, Box<dyn std::error::Error>> {
        // Load config from file
        Ok(Config::default())
    }
}
