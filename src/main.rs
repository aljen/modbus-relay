use clap::{Args, Parser};
use std::{path::PathBuf, sync::Arc};
use tracing::{error, info};

use modbus_relay::{setup_logging, ModbusRelay, RelayConfig};

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
    // Parse command line args
    let cli = Cli::parse();

    if cli.common.dump_default {
        let config = RelayConfig::default();
        println!("{}", serde_json::to_string_pretty(&config)?);
        return Ok(());
    }

    // Load and validate configuration
    let config = if cli.common.dump_default {
        println!("{}", serde_json::to_string_pretty(&RelayConfig::default())?);
        return Ok(());
    } else {
        RelayConfig::load(Some(&cli.common.config))?
    };

    setup_logging(&config)?;

    // Create and run relay
    let relay = Arc::new(ModbusRelay::new(config)?);
    let relay_clone = Arc::clone(&relay);

    // Handle shutdown signals
    tokio::spawn(async move {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to create SIGTERM signal handler");
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("Failed to create SIGINT signal handler");

        tokio::select! {
            _ = sigterm.recv() => info!("Received SIGTERM"),
            _ = sigint.recv() => info!("Received SIGINT"),
        }

        info!("Starting graceful shutdown");
        if let Err(e) = relay_clone.shutdown().await {
            error!("Error during shutdown: {}", e);
        }
    });

    relay.run().await?;

    Ok(())
}
