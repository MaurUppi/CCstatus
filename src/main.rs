use ccstatus::cli::Cli;
use ccstatus::config::{Config, InputData};
use ccstatus::core::{collect_all_segments, StatusLineGenerator};
use std::io;

#[cfg(feature = "network-monitoring")]
use ccstatus::core::network::StatuslineInput;

#[cfg(feature = "network-monitoring")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    main_impl().await
}

#[cfg(not(feature = "network-monitoring"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    futures::executor::block_on(main_impl())
}

async fn main_impl() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse_args();

    // Handle configuration commands

    if cli.update {
        #[cfg(feature = "self-update")]
        {
            println!("Update feature not implemented in new architecture yet");
        }
        #[cfg(not(feature = "self-update"))]
        {
            println!("Update check not available (self-update feature disabled)");
        }
        return Ok(());
    }

    // Load configuration
    let config = Config::load().unwrap_or_else(|_| Config::default());

    // Read Claude Code data from stdin with two-tier data flow for network monitoring
    let stdin = io::stdin();
    
    #[cfg(feature = "network-monitoring")]
    let (input, full_input) = {
        let full_input: StatuslineInput = serde_json::from_reader(stdin.lock())?;
        let input = InputData::from(&full_input);
        (input, Some(full_input))
    };
    
    #[cfg(not(feature = "network-monitoring"))]
    let (input, full_input) = {
        let input: InputData = serde_json::from_reader(stdin.lock())?;
        (input, None::<()>)
    };

    // Collect segment data
    let segments_data = collect_all_segments(&config, &input, full_input.as_ref()).await;

    // Render statusline
    let generator = StatusLineGenerator::new(config);
    let statusline = generator.generate(segments_data);

    println!("{}", statusline);

    Ok(())
}
