use ccometixline::cli::Cli;
use ccometixline::config::{Config, ConfigLoader, InputData};
use ccometixline::core::StatusLineGenerator;
use std::io;

fn main() -> io::Result<()> {
    let cli = Cli::parse_args();

    // Handle special CLI modes
    if cli.print_config {
        let config = Config::default();
        println!("{}", toml::to_string(&config).unwrap());
        return Ok(());
    }

    if cli.validate {
        println!("Configuration validation not implemented yet");
        return Ok(());
    }

    if cli.configure {
        println!("TUI configuration mode not implemented yet");
        return Ok(());
    }

    // Load configuration
    let config = ConfigLoader::load();

    // Read Claude Code data from stdin
    let stdin = io::stdin();
    let input: InputData = serde_json::from_reader(stdin.lock())?;

    // Generate statusline
    let generator = StatusLineGenerator::new(config);
    let statusline = generator.generate(&input);

    println!("{}", statusline);

    Ok(())
}
