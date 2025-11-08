use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use clap::Parser;
use time::UtcOffset;
use tracing::{error, info};
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{
    EnvFilter, Layer, Registry, fmt::time::OffsetTime, layer::SubscriberExt,
    util::SubscriberInitExt,
};

use modbus_relay::{
    ModbusRelay, RelayConfig, RelayError, TransportError, errors::InitializationError,
};

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

pub fn setup_logging(
    config: &RelayConfig,
) -> Result<(impl Drop + use<>, impl Drop + use<>), RelayError> {
    let timer = OffsetTime::new(
        UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
        time::format_description::well_known::Rfc3339,
    );

    let log_dir = PathBuf::from(&config.logging.log_dir);
    let include_location = config.logging.include_location;
    let thread_ids = config.logging.thread_ids;
    let thread_names = config.logging.thread_names;

    std::fs::create_dir_all(&log_dir).unwrap_or_else(|_| {
        eprintln!("Failed to create log directory {}", log_dir.display());
        process::exit(1);
    });

    // Non-blocking stdout
    let (stdout_writer, stdout_guard) = non_blocking(std::io::stdout());

    // Rotating log writer
    let file_appender = rolling::daily(log_dir, "modbus-relay.log");
    let (file_writer, file_guard) = non_blocking(file_appender);

    // Environment-based filter
    let mut stdout_env_filter = EnvFilter::builder()
        .with_default_directive(config.logging.get_level_filter().into())
        .from_env_lossy();

    let mut file_env_filter = EnvFilter::builder()
        .with_default_directive(config.logging.get_level_filter().into())
        .from_env_lossy();

    if config.logging.trace_frames {
        stdout_env_filter = stdout_env_filter
            .add_directive("modbus_relay::protocol=trace".parse().unwrap())
            .add_directive("modbus_relay::transport=trace".parse().unwrap());

        file_env_filter = file_env_filter
            .add_directive("modbus_relay::protocol=trace".parse().unwrap())
            .add_directive("modbus_relay::transport=trace".parse().unwrap());
    }

    // Log layer for stdout
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(stdout_writer)
        .with_target(false)
        .with_thread_ids(thread_ids)
        .with_thread_names(thread_names)
        .with_file(include_location)
        .with_line_number(include_location)
        .with_level(true)
        .with_timer(timer.clone())
        .with_filter(stdout_env_filter);

    // Log layer for file
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_target(false)
        .with_thread_ids(thread_ids)
        .with_thread_names(thread_names)
        .with_file(include_location)
        .with_line_number(include_location)
        .with_level(true)
        .with_timer(timer)
        .with_filter(file_env_filter);

    // Combine all layers
    Registry::default()
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .map_err(|e| {
            RelayError::Init(InitializationError::logging(format!(
                "Failed to initialize logging: {}",
                e
            )))
        })?;

    Ok((stdout_guard, file_guard))
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Load configuration
    let config = if let Some(config_path) = &cli.common.config {
        RelayConfig::from_file(config_path.clone())
    } else {
        RelayConfig::new()
    };

    let config = match config {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {:#}", e);
            process::exit(1);
        }
    };

    // Setup logging based on configuration
    let (_stdout_guard, _file_guard) = match setup_logging(&config) {
        Ok(guards) => guards,
        Err(e) => {
            eprintln!("Failed to setup logging: {:#}", e);
            process::exit(1);
        }
    };

    info!("Starting Modbus Relay...");

    if let Err(e) = run(config).await {
        error!("Fatal error: {:#}", e);
        if let Some(RelayError::Transport(TransportError::Io { details, .. })) =
            e.downcast_ref::<RelayError>()
            && details.contains("serial port")
        {
            error!(
                "Hint: Make sure the configured serial port exists and you have permission to access it"
            );
            #[cfg(target_os = "macos")]
            error!(
                "Hint: On macOS, you might need to install the driver from https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers"
            );
            #[cfg(target_os = "linux")]
            error!(
                "Hint: On Linux, you might need to add your user to the dialout group: sudo usermod -a -G dialout $USER"
            );
        }
        process::exit(1);
    }
}

async fn run(config: RelayConfig) -> Result<(), Box<dyn std::error::Error>> {
    let relay = Arc::new(ModbusRelay::new(config)?);

    let relay_clone = Arc::clone(&relay);

    let shutdown_task = tokio::spawn(async move {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to create SIGTERM signal handler");
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("Failed to create SIGINT signal handler");
        tokio::select! {
            _ = sigterm.recv() => info!("Received SIGTERM"),
            _ = sigint.recv() => info!("Received SIGINT"),
        }

        if let Err(e) = relay_clone.shutdown().await {
            error!("Error during shutdown: {}", e);
        }
    });

    relay.run().await?;

    info!("Waiting for shutdown to complete...");

    shutdown_task.await?;

    info!("Modbus Relay stopped");

    Ok(())
}
