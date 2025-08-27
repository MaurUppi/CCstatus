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
            use crate::config::{
                AnsiColor, ColorConfig, IconConfig, SegmentConfig, SegmentId, StyleConfig,
                StyleMode, TextStyleConfig,
            };

            let mut segments = vec![
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
                    styles: TextStyleConfig { text_bold: false },
                    options: std::collections::HashMap::new(),
                },
                SegmentConfig {
                    id: SegmentId::Directory,
                    enabled: true,
                    icon: IconConfig {
                        plain: "D".to_string(),
                        nerd_font: "󰉋".to_string(),
                    },
                    colors: ColorConfig {
                        icon: Some(AnsiColor::Color16 { c16: 4 }), // Blue
                        text: Some(AnsiColor::Color16 { c16: 7 }), // White
                        background: None,
                    },
                    styles: TextStyleConfig { text_bold: false },
                    options: std::collections::HashMap::new(),
                },
                SegmentConfig {
                    id: SegmentId::Git,
                    enabled: true,
                    icon: IconConfig {
                        plain: "G".to_string(),
                        nerd_font: "󰊢".to_string(),
                    },
                    colors: ColorConfig {
                        icon: Some(AnsiColor::Color16 { c16: 3 }), // Yellow
                        text: Some(AnsiColor::Color16 { c16: 7 }), // White
                        background: None,
                    },
                    styles: TextStyleConfig { text_bold: false },
                    options: std::collections::HashMap::new(),
                },
                SegmentConfig {
                    id: SegmentId::Usage,
                    enabled: true,
                    icon: IconConfig {
                        plain: "U".to_string(),
                        nerd_font: "󰾆".to_string(),
                    },
                    colors: ColorConfig {
                        icon: Some(AnsiColor::Color16 { c16: 5 }), // Magenta
                        text: Some(AnsiColor::Color16 { c16: 7 }), // White
                        background: None,
                    },
                    styles: TextStyleConfig { text_bold: false },
                    options: std::collections::HashMap::new(),
                },
            ];

            // Add network segment when network-monitoring feature is enabled
            #[cfg(feature = "network-monitoring")]
            segments.push(SegmentConfig {
                id: SegmentId::Network,
                enabled: true,
                icon: IconConfig {
                    plain: "".to_string(),
                    nerd_font: "".to_string(),
                },
                colors: ColorConfig {
                    icon: Some(AnsiColor::Color16 { c16: 10 }), // Green
                    text: Some(AnsiColor::Color16 { c16: 10 }),
                    background: None,
                },
                styles: TextStyleConfig { text_bold: false },
                options: std::collections::HashMap::new(),
            });

            Config {
                theme: "default".to_string(),
                style: StyleConfig {
                    mode: StyleMode::Plain,
                    separator: " | ".to_string(),
                },
                segments,
            }
        }
    }
}
