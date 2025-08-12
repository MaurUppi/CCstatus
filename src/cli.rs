use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "CCometixLine (ccline)")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(
    about = "CCometixLine (ccline) - High-performance Claude Code StatusLine tool written in Rust"
)]
#[command(
    long_about = concat!(
        "CCometixLine (ccline) v", env!("CARGO_PKG_VERSION"), "\n",
        "A high-performance Claude Code StatusLine tool written in Rust.\n",
        "Provides real-time usage tracking, Git integration, and customizable themes."
    )
)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<String>,

    /// Theme selection
    #[arg(short, long, default_value = "dark")]
    pub theme: String,

    /// Enable TUI configuration mode
    #[arg(long)]
    pub configure: bool,

    /// Print default configuration
    #[arg(long)]
    pub print_config: bool,

    /// Validate configuration file
    #[arg(long)]
    pub validate: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
