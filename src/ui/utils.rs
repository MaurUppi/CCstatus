use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
};

/// Create a centered rectangle with the given percentage dimensions
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Convert ANSI color code (0-15) to ratatui Color
pub fn ansi_to_ratatui_color(ansi: u8) -> Color {
    match ansi {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::White,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::Gray,
        _ => Color::White,
    }
}

/// Convert AnsiColor to ratatui Color
pub fn ansi_color_to_ratatui_color(ansi_color: &crate::config::AnsiColor) -> Color {
    match ansi_color {
        crate::config::AnsiColor::Color16 { c16 } => ansi_to_ratatui_color(*c16),
        crate::config::AnsiColor::Color256 { c256 } => Color::Indexed(*c256),
        crate::config::AnsiColor::Rgb { r, g, b } => Color::Rgb(*r, *g, *b),
    }
}

/// Convert AnsiColor to descriptive string
pub fn ansi_color_to_description(ansi_color: Option<&crate::config::AnsiColor>) -> String {
    match ansi_color {
        Some(crate::config::AnsiColor::Color16 { c16 }) => match c16 {
            0 => "Black".to_string(),
            1 => "Red".to_string(),
            2 => "Green".to_string(),
            3 => "Yellow".to_string(),
            4 => "Blue".to_string(),
            5 => "Magenta".to_string(),
            6 => "Cyan".to_string(),
            7 => "White".to_string(),
            8 => "Dark Gray".to_string(),
            9 => "Light Red".to_string(),
            10 => "Light Green".to_string(),
            11 => "Light Yellow".to_string(),
            12 => "Light Blue".to_string(),
            13 => "Light Magenta".to_string(),
            14 => "Light Cyan".to_string(),
            15 => "Gray".to_string(),
            _ => format!("ANSI {}", c16),
        },
        Some(crate::config::AnsiColor::Color256 { c256 }) => format!("256:{}", c256),
        Some(crate::config::AnsiColor::Rgb { r, g, b }) => format!("RGB({},{},{})", r, g, b),
        None => "Default".to_string(),
    }
}