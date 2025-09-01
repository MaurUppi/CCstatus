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
            use ccstatus::updater::{geo, manifest::ManifestClient, url_resolver};

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

            // Resolve URLs for sequential trying with persistent caching
            let urls = url_resolver::resolve_manifest_url(is_china);
            let mut client = ManifestClient::new();
            let mut update_found = false;

            // Check for verbose debug output
            let debug_enabled = std::env::var("CCSTATUS_DEBUG").is_ok();

            if debug_enabled {
                eprintln!(
                    "Update check: trying {} URLs for {} region",
                    urls.len(),
                    if is_china { "China" } else { "global" }
                );
            }

            // Use improved sequential URL trying with better error reporting
            match url_resolver::try_urls_in_sequence(&urls, |url| {
                if debug_enabled {
                    eprintln!("Trying: {}", url);
                }

                let result = client.fetch_manifest_with_persistent_cache(
                    url,
                    &state.etag_map,
                    &state.last_modified_map,
                )?;

                Ok((url.to_string(), result))
            }) {
                Ok((successful_url, (manifest_opt, new_etag, new_last_modified))) => {
                    if debug_enabled {
                        eprintln!("Success: {}", successful_url);
                    }

                    if manifest_opt.is_none() {
                        // 304 Not Modified - no update available, short-circuit
                        eprintln!("You have the latest version");
                        if debug_enabled {
                            eprintln!("Debug: No update available (304 Not Modified)");
                        }
                        std::process::exit(0);
                    }

                    let manifest = manifest_opt.unwrap();

                    // Update persistent cache if we have new headers
                    let host = url_resolver::extract_host_from_url(&successful_url)
                        .unwrap_or_else(|| successful_url);
                    let mut cache_updated = false;

                    if let Some(etag) = new_etag {
                        state.etag_map.insert(host.clone(), etag);
                        cache_updated = true;
                    }
                    if let Some(last_modified) = new_last_modified {
                        state.last_modified_map.insert(host, last_modified);
                        cache_updated = true;
                    }

                    if cache_updated {
                        state.save().ok();
                    }

                    // Check if newer version available
                    if client.is_newer_version(&manifest.version).unwrap_or(false) {
                        // Check if blinking output is enabled (default: true)
                        let flash_enabled = std::env::var("CCSTATUS_FLASH")
                            .map(|v| v.to_lowercase() != "0" && v.to_lowercase() != "false")
                            .unwrap_or(true);

                        let output = if flash_enabled {
                            format!(
                                "\x1b[5m v{} released \x1b[0m ({})",
                                manifest.version, manifest.notes_url
                            )
                        } else {
                            format!("v{} released ({})", manifest.version, manifest.notes_url)
                        };
                        eprintln!("{}", output);
                        update_found = true;
                    }

                    if update_found {
                        std::process::exit(10);
                    } else {
                        eprintln!("You have the latest version");
                        if debug_enabled {
                            eprintln!("Debug: No update available (current version is latest)");
                        }
                        std::process::exit(0);
                    }
                }
                Err(error) => {
                    // All URLs failed - provide detailed error message
                    if debug_enabled {
                        eprintln!("Update check failed: {}", error);
                    } else {
                        eprintln!("Failed to check for updates: {}", error);
                    }
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
