use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "High-performance Claude Code StatusLine with Network Probe")]
#[command(version = concat!("Ver:", env!("CARGO_PKG_VERSION")))]
#[command(about = "High-performance Claude Code StatusLine with Network Probe")]
pub struct Cli {
    /// Check for updates
    #[arg(short = 'u', long = "update")]
    pub update: bool,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
