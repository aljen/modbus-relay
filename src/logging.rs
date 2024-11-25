use time::UtcOffset;
use tracing_subscriber::{
    fmt::time::OffsetTime, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
    Registry,
};

use crate::{errors::InitializationError, RelayConfig, RelayError};

pub fn setup_logging(config: &RelayConfig) -> Result<(), RelayError> {
    // Validate logging config before proceeding
    config.log.validate().map_err(RelayError::Init)?;

    let timer = OffsetTime::new(
        UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
        time::format_description::well_known::Rfc3339,
    );

    // Determine base level filter
    let base_level = config.log.get_level_filter();

    // Build the EnvFilter
    let mut env_filter = EnvFilter::default().add_directive(base_level.into());

    // If trace_frames is enabled, add more specific filtering
    if config.log.trace_frames {
        env_filter = env_filter
            .add_directive("modbus_relay::protocol=trace".parse().unwrap())
            .add_directive("modbus_relay::transport=trace".parse().unwrap());
    }

    // Build and initialize the subscriber
    let layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(config.log.include_location)
        .with_line_number(config.log.include_location)
        .with_level(true)
        .with_timer(timer)
        .with_filter(env_filter);

    Registry::default().with(layer).try_init().map_err(|e| {
        RelayError::Init(InitializationError::logging(format!(
            "Failed to initialize logging: {}",
            e
        )))
    })?;

    Ok(())
}

// Helper for creating request identifiers
pub fn generate_request_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    format!("{:x}", rng.gen::<u64>())
}

#[cfg(test)]
mod tests {
    use tracing::level_filters::LevelFilter;

    use crate::relay_config::LogConfig;

    #[test]
    fn test_log_config_validation() {
        let config = LogConfig {
            log_level: "invalid".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());

        let config = LogConfig {
            log_level: "debug".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_level_filter() {
        let config = LogConfig {
            log_level: "debug".to_string(),
            ..Default::default()
        };
        assert_eq!(config.get_level_filter(), LevelFilter::DEBUG);

        let config = LogConfig {
            log_level: "invalid".to_string(),
            ..Default::default()
        };
        assert_eq!(config.get_level_filter(), LevelFilter::INFO); // fallback
    }
}
