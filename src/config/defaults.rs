// Legacy defaults - now using ui/themes/presets.rs for configuration
// This file kept for backward compatibility

use super::types::Config;

impl Default for Config {
    fn default() -> Self {
        // Use theme presets when TUI feature is available
        #[cfg(feature = "tui")]
        {
            crate::ui::themes::ThemePresets::get_default()
        }
        
        // Fallback minimal config when TUI feature is disabled
        #[cfg(not(feature = "tui"))]
        {
            use crate::config::{AnsiColor, SegmentConfig, ColorConfig, IconConfig, SegmentId, TextStyleConfig, StyleConfig, StyleMode};
            
            Config {
                theme: "default".to_string(),
                style: StyleConfig {
                    mode: StyleMode::Plain,
                    separator: "|".to_string(),
                },
                segments: vec![
                    SegmentConfig {
                        id: SegmentId::Model,
                        enabled: true,
                        icon: IconConfig {
                            plain: "M".to_string(),
                            nerd_font: "󰧑".to_string(),
                        },
                        colors: ColorConfig {
                            icon: Some(AnsiColor::Color16 { c16: 6 }), // Cyan
                            text: Some(AnsiColor::Color16 { c16: 7 }), // White
                            background: None,
                        },
                        styles: TextStyleConfig {
                            text_bold: false,
                        },
                        options: std::collections::HashMap::new(),
                    },
                ],
            }
        }
    }
}
