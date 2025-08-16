use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "ccline")]
#[command(version, about = "High-performance Claude Code StatusLine")]
pub struct Cli {
    /// Enter TUI configuration mode
    #[arg(short = 'c', long = "config")]
    pub config: bool,

    /// Set theme
    #[arg(short = 't', long = "theme")]
    pub theme: Option<String>,

    /// Print current configuration
    #[arg(long = "print")]
    pub print: bool,

    /// Initialize config file
    #[arg(long = "init")]
    pub init: bool,

    /// Check configuration
    #[arg(long = "check")]
    pub check: bool,

    /// Check for updates
    #[arg(short = 'u', long = "update")]
    pub update: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
