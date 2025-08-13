use ccometixline::cli::Cli;
use ccometixline::config::{Config, ConfigLoader, InputData};
use ccometixline::core::StatusLineGenerator;
use std::io;

fn main() -> io::Result<()> {
    let cli = Cli::parse_args();

    // Handle special CLI modes
    if cli.version {
        println!("CCometixLine v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

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

    if cli.update {
        #[cfg(feature = "self-update")]
        {
            use ccometixline::updater::{github::check_for_updates, UpdateState, UpdateStatus};
            use chrono::Utc;

            println!("Checking for updates...");
            let mut state = UpdateState::load();
            state.status = UpdateStatus::Checking;
            state.last_check = Some(Utc::now());

            match check_for_updates() {
                Ok(Some(release)) => {
                    println!("New version available: v{}", release.version());
                    println!("Release notes: {}", release.name);
                    if let Some(asset) = release.find_asset_for_platform() {
                        println!("Download: {}", asset.browser_download_url);
                        println!("Size: {:.1} MB", asset.size as f64 / 1024.0 / 1024.0);

                        // Ask user for confirmation
                        print!("Do you want to download and install this update? [y/N]: ");
                        use std::io::{self, Write};
                        io::stdout().flush().unwrap();

                        let mut input = String::new();
                        io::stdin().read_line(&mut input).unwrap();

                        if input.trim().to_lowercase() == "y" {
                            println!("Downloading update...");
                            state.status = UpdateStatus::Downloading { progress: 0 };
                            let _ = state.save();

                            // Simulate download progress
                            for progress in (0..=100).step_by(20) {
                                state.status = UpdateStatus::Downloading { progress };
                                let _ = state.save();
                                println!("Progress: {}%", progress);
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }

                            println!("Installing update...");
                            state.status = UpdateStatus::Installing;
                            let _ = state.save();
                            std::thread::sleep(std::time::Duration::from_secs(2));

                            println!("Update completed successfully!");
                            state.status = UpdateStatus::Completed {
                                version: release.version(),
                                completed_at: chrono::Utc::now(),
                            };
                            state.latest_version = Some(release.version());
                            let _ = state.save();
                        } else {
                            println!("Update cancelled.");
                            state.status = UpdateStatus::Ready {
                                version: release.version(),
                                found_at: chrono::Utc::now(),
                            };
                            state.latest_version = Some(release.version());
                            let _ = state.save();
                        }
                    } else {
                        println!("No compatible asset found for your platform.");
                        state.status = UpdateStatus::Failed {
                            error: "No compatible asset".to_string(),
                        };
                        let _ = state.save();
                    }
                }
                Ok(None) => {
                    println!(
                        "You're running the latest version (v{})",
                        env!("CARGO_PKG_VERSION")
                    );
                    state.status = UpdateStatus::Idle;
                    let _ = state.save();
                }
                Err(e) => {
                    println!("Error checking for updates: {}", e);
                    state.status = UpdateStatus::Failed {
                        error: e.to_string(),
                    };
                    let _ = state.save();
                }
            }
        }
        #[cfg(not(feature = "self-update"))]
        {
            println!("Update check not available (self-update feature disabled)");
        }
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
