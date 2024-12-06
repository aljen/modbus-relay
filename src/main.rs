use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use clap::Parser;
use tracing::{error, info};

use modbus_relay::{ModbusRelay, RelayConfig, RelayError, TransportError};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    common: CommonArgs,
}

#[derive(clap::Args)]
#[group(multiple = false)]
struct CommonArgs {
    /// Path to config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Run in debug mode
    #[arg(short, long)]
    debug: bool,
}

pub fn setup_logging(config: Option<&RelayConfig>) -> Result<(), RelayError> {
    use modbus_relay::errors::InitializationError;
    use modbus_relay::RelayError;
    use time::UtcOffset;
    use tracing_subscriber::fmt::time::OffsetTime;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::{filter::LevelFilter, EnvFilter, Layer, Registry};

    let timer = OffsetTime::new(
        UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
        time::format_description::well_known::Rfc3339,
    );

    // Configure log level filter
    let mut env_filter = EnvFilter::default();

    // Configure based on config
    let include_location = if let Some(cfg) = config {
        // Use configured log level
        env_filter = env_filter.add_directive(cfg.logging.get_level_filter().into());

        if cfg.logging.trace_frames {
            env_filter = env_filter
                .add_directive("modbus_relay::protocol=trace".parse().unwrap())
                .add_directive("modbus_relay::transport=trace".parse().unwrap());
        }

        cfg.logging.include_location
    } else {
        // Use INFO level for startup
        env_filter = env_filter.add_directive(LevelFilter::INFO.into());
        true
    };

    // Build subscriber with all configuration
    let layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_timer(timer)
        .with_file(include_location)
        .with_line_number(include_location)
        .with_level(true)
        .with_filter(env_filter);

    let subscriber = Registry::default().with(layer);

    // Set up global subscriber only once
    static LOGGER_INITIALIZED: std::sync::Once = std::sync::Once::new();
    let mut error = None;

    LOGGER_INITIALIZED.call_once(|| {
        if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
            error = Some(RelayError::Init(InitializationError::logging(format!(
                "Failed to initialize logging: {}",
                e
            ))));
        }
    });

    if let Some(e) = error {
        return Err(e);
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        error!("Fatal error: {:#}", e);
        if let Some(RelayError::Transport(TransportError::Io { details, .. })) =
            e.downcast_ref::<RelayError>()
        {
            if details.contains("serial port") {
                error!("Hint: Make sure the configured serial port exists and you have permission to access it");
                #[cfg(target_os = "macos")]
                error!("Hint: On macOS, you might need to install the driver from https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers");
                #[cfg(target_os = "linux")]
                error!("Hint: On Linux, you might need to add your user to the dialout group: sudo usermod -a -G dialout $USER");
            }
        }
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    setup_logging(None)?;

    info!("Starting Modbus Relay...");

    let cli = Cli::parse();

    // Initialize logging
    let config = if let Some(config_path) = &cli.common.config {
        RelayConfig::from_file(config_path.clone())?
    } else {
        RelayConfig::new()?
    };

    // Setup logging based on configuration
    setup_logging(Some(&config))?;

    let relay = Arc::new(ModbusRelay::new(config)?);
    relay.run().await?;

    Ok(())
}
