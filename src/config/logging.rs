use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "pretty".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Enable trace-level logging for frame contents
    #[serde(default)]
    pub trace_frames: bool,

    /// Minimum log level for console output
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Log format (pretty or json)
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Whether to include source code location in logs
    #[serde(default = "default_true")]
    pub include_location: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            trace_frames: false,
            log_level: default_log_level(),
            format: default_log_format(),
            include_location: default_true(),
        }
    }
}

impl Config {
    pub fn get_level_filter(&self) -> LevelFilter {
        match self.log_level.to_lowercase().as_str() {
            "error" => LevelFilter::ERROR,
            "warn" => LevelFilter::WARN,
            "info" => LevelFilter::INFO,
            "debug" => LevelFilter::DEBUG,
            "trace" => LevelFilter::TRACE,
            _ => LevelFilter::INFO, // Fallback to INFO if invalid
        }
    }
}
