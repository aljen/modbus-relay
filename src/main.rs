use clap::{Args, Parser};
use std::{path::PathBuf, sync::Arc};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use modbus_relay::{ModbusRelay, RelayConfig};

#[derive(Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(Args)]
struct CommonArgs {
    /// Path to the config file
    #[arg(short, long, default_value = "/etc/modbus-relay.json")]
    config: PathBuf,

    /// Dump default config and exit
    #[arg(long = "dump-default-config")]
    dump_default: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .with_line_number(true)
        .with_file(true)
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_ansi(true)
        .pretty()
        .init();

    // Parse command line args
    let cli = Cli::parse();

    if cli.common.dump_default {
        let config = RelayConfig::default();
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }

    // Load config
    let config = if cli.common.config.exists() {
        info!("Loading config from {}", cli.common.config.display());
        let content = std::fs::read_to_string(&cli.common.config)?;
        let config: RelayConfig = serde_json::from_str(&content)?;
        config
            .validate()
            .map_err(|err| modbus_relay::RelayError::Config {
                kind: err.kind,
                details: err.details,
            })?;
        config
    } else {
        info!("Config file not found, using defaults");
        info!(
            "Consider running with --dump-default-config > {}",
            cli.common.config.display()
        );
        RelayConfig::default()
    };

    // Create and run relay
    let relay = Arc::new(ModbusRelay::new(config)?);
    relay.run().await?;

    Ok(())
}
