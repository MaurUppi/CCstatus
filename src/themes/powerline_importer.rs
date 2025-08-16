use crate::config::{AnsiColor, Config, SegmentConfig, SegmentId, ColorConfig, StyleConfig, IconConfig, StylesConfig, StyleMode};

/// Convert hex color string to RGB AnsiColor
fn hex_to_rgb(hex: &str) -> Result<AnsiColor, String> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Err(format!("Invalid hex color: #{}", hex));
    }
    
    let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid red component")?;
    let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid green component")?;
    let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid blue component")?;
    
    Ok(AnsiColor::Rgb { r, g, b })
}

/// Create a Powerline-compatible theme from claude-powerline color scheme
pub fn create_powerline_theme(
    theme_name: &str,
    directory_colors: (&str, &str),
    git_colors: (&str, &str),
    model_colors: (&str, &str),
    usage_colors: Option<(&str, &str)>,
    update_colors: Option<(&str, &str)>,
) -> Result<Config, String> {
    let mut config = Config::default();
    
    // Set theme name and use Powerline separator
    config.theme = theme_name.to_string();
    config.style.separator = "\u{e0b0}".to_string();
    config.style.mode = StyleMode::NerdFont;
    
    // Create segments with Powerline colors
    let mut segments = Vec::new();
    
    // Model segment
    segments.push(SegmentConfig {
        id: SegmentId::Model,
        enabled: true,
        icon: IconConfig {
            plain: "🔮".to_string(),
            nerd_font: "\u{f02a2}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(hex_to_rgb(model_colors.1)?),
            text: Some(hex_to_rgb(model_colors.1)?),
            background: Some(hex_to_rgb(model_colors.0)?),
        },
        styles: StylesConfig {
            text_bold: false,
        },
        options: std::collections::HashMap::new(),
    });
    
    // Directory segment
    segments.push(SegmentConfig {
        id: SegmentId::Directory,
        enabled: true,
        icon: IconConfig {
            plain: "📁".to_string(),
            nerd_font: "\u{f115}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(hex_to_rgb(directory_colors.1)?),
            text: Some(hex_to_rgb(directory_colors.1)?),
            background: Some(hex_to_rgb(directory_colors.0)?),
        },
        styles: StylesConfig {
            text_bold: false,
        },
        options: std::collections::HashMap::new(),
    });
    
    // Git segment
    segments.push(SegmentConfig {
        id: SegmentId::Git,
        enabled: true,
        icon: IconConfig {
            plain: "🔗".to_string(),
            nerd_font: "\u{f1d3}".to_string(),
        },
        colors: ColorConfig {
            icon: Some(hex_to_rgb(git_colors.1)?),
            text: Some(hex_to_rgb(git_colors.1)?),
            background: Some(hex_to_rgb(git_colors.0)?),
        },
        styles: StylesConfig {
            text_bold: false,
        },
        options: std::collections::HashMap::new(),
    });
    
    // Usage segment (if provided)
    if let Some(usage_colors) = usage_colors {
        segments.push(SegmentConfig {
            id: SegmentId::Usage,
            enabled: true,
            icon: IconConfig {
                plain: "💰".to_string(),
                nerd_font: "\u{f111}".to_string(),
            },
            colors: ColorConfig {
                icon: Some(hex_to_rgb(usage_colors.1)?),
                text: Some(hex_to_rgb(usage_colors.1)?),
                background: Some(hex_to_rgb(usage_colors.0)?),
            },
            styles: StylesConfig {
                text_bold: false,
            },
            options: std::collections::HashMap::new(),
        });
    }
    
    // Update segment (if provided)
    if let Some(update_colors) = update_colors {
        segments.push(SegmentConfig {
            id: SegmentId::Update,
            enabled: true,
            icon: IconConfig {
                plain: "⬆️".to_string(),
                nerd_font: "\u{f062}".to_string(),
            },
            colors: ColorConfig {
                icon: Some(hex_to_rgb(update_colors.1)?),
                text: Some(hex_to_rgb(update_colors.1)?),
                background: Some(hex_to_rgb(update_colors.0)?),
            },
            styles: StylesConfig {
                text_bold: false,
            },
            options: std::collections::HashMap::new(),
        });
    }
    
    config.segments = segments;
    Ok(config)
}

/// Generate all Powerline themes based on claude-powerline configurations
pub fn generate_powerline_themes() -> Result<Vec<(String, Config)>, String> {
    let mut themes = Vec::new();
    
    // Dark theme
    let dark_theme = create_powerline_theme(
        "powerline-dark",
        ("#8b4513", "#ffffff"), // directory: brown bg, white fg
        ("#404040", "#ffffff"), // git: dark gray bg, white fg  
        ("#2d2d2d", "#ffffff"), // model: darker gray bg, white fg
        Some(("#374151", "#d1d5db")), // usage: metrics colors
        Some(("#3a3a4a", "#b8b8d0")), // update: version colors
    )?;
    themes.push(("powerline-dark".to_string(), dark_theme));
    
    // Light theme  
    let light_theme = create_powerline_theme(
        "powerline-light",
        ("#ff6b47", "#ffffff"), // directory: coral bg, white fg
        ("#4fb3d9", "#ffffff"), // git: sky blue bg, white fg
        ("#87ceeb", "#000000"), // model: sky blue bg, black fg
        Some(("#6b7280", "#ffffff")), // usage: gray bg, white fg
        Some(("#8b7dd8", "#ffffff")), // update: purple bg, white fg
    )?;
    themes.push(("powerline-light".to_string(), light_theme));
    
    // Nord theme
    let nord_theme = create_powerline_theme(
        "powerline-nord",
        ("#434c5e", "#d8dee9"), // directory: nord gray bg, light fg
        ("#3b4252", "#a3be8c"), // git: nord dark bg, green fg
        ("#4c566a", "#81a1c1"), // model: nord blue-gray bg, blue fg
        Some(("#b48ead", "#2e3440")), // usage: nord purple bg, dark fg
        Some(("#434c5e", "#88c0d0")), // update: nord gray bg, cyan fg
    )?;
    themes.push(("powerline-nord".to_string(), nord_theme));
    
    // Tokyo Night theme
    let tokyo_night_theme = create_powerline_theme(
        "powerline-tokyo-night",
        ("#2f334d", "#82aaff"), // directory: tokyo blue-gray bg, blue fg
        ("#1e2030", "#c3e88d"), // git: tokyo dark bg, green fg
        ("#191b29", "#fca7ea"), // model: tokyo darker bg, pink fg
        Some(("#3d59a1", "#c0caf5")), // usage: tokyo blue bg, light fg
        Some(("#292e42", "#bb9af7")), // update: tokyo purple-gray bg, purple fg
    )?;
    themes.push(("powerline-tokyo-night".to_string(), tokyo_night_theme));
    
    // Rose Pine theme
    let rose_pine_theme = create_powerline_theme(
        "powerline-rose-pine",
        ("#26233a", "#c4a7e7"), // directory: rose dark bg, purple fg
        ("#1f1d2e", "#9ccfd8"), // git: rose darker bg, cyan fg
        ("#191724", "#ebbcba"), // model: rose darkest bg, pink fg
        Some(("#524f67", "#e0def4")), // usage: rose gray bg, light fg
        Some(("#2a273f", "#c4a7e7")), // update: rose medium bg, purple fg
    )?;
    themes.push(("powerline-rose-pine".to_string(), rose_pine_theme));
    
    Ok(themes)
}