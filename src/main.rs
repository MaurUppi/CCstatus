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
    
    // Handle check-update command
    if cli.check_update {
        #[cfg(feature = "self-update")]
        {
            use ccstatus::updater::{geo, url_resolver, manifest::ManifestClient};
            
            // Perform immediate update check
            let mut state = ccstatus::updater::UpdateStateFile::load();
            
            // Get geographic detection
            let is_china = if state.is_geo_verdict_valid() {
                state.geo_verdict.unwrap_or(false)
            } else {
                let detected = geo::detect_china_ttl24h();
                state.update_geo_verdict(detected);
                state.save().ok();
                detected
            };
            
            // Resolve URLs and try to fetch manifest
            let urls = url_resolver::resolve_manifest_url(is_china);
            let mut client = ManifestClient::new();
            
            match url_resolver::try_urls_in_sequence(&urls, |url| {
                client.fetch_manifest(url)
            }) {
                Ok(Some(manifest)) => {
                    if client.is_newer_version(&manifest.version).unwrap_or(false) {
                        eprintln!(" v{} released ", manifest.version);
                        std::process::exit(10);
                    } else {
                        std::process::exit(0);
                    }
                }
                Ok(None) => {
                    // No new version (304 not modified)
                    std::process::exit(0);
                }
                Err(_) => {
                    eprintln!("Failed to check for updates");
                    std::process::exit(1);
                }
            }
        }
        #[cfg(not(feature = "self-update"))]
        {
            eprintln!("Update check not available (self-update feature disabled)");
            std::process::exit(1);
        }
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
