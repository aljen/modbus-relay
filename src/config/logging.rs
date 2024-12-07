use serde::{Deserialize, Serialize};
use tracing::level_filters::LevelFilter;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Directory to store log files
    pub log_dir: String,

    /// Enable trace-level logging for frame contents
    pub trace_frames: bool,

    /// Minimum log level for console output
    pub level: String,

    /// Log format (pretty or json)
    pub format: String,

    /// Whether to include source code location in logs
    pub include_location: bool,

    /// Whether to include thread IDs in logs
    pub thread_ids: bool,

    /// Whether to include thread names in logs
    pub thread_names: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_dir: "logs".to_string(),
            trace_frames: false,
            level: "info".to_string(),
            format: "pretty".to_string(),
            include_location: false,
            thread_ids: false,
            thread_names: false,
        }
    }
}

impl Config {
    pub fn get_level_filter(&self) -> LevelFilter {
        match self.level.to_lowercase().as_str() {
            "error" => LevelFilter::ERROR,
            "warn" => LevelFilter::WARN,
            "info" => LevelFilter::INFO,
            "debug" => LevelFilter::DEBUG,
            "trace" => LevelFilter::TRACE,
            _ => LevelFilter::INFO, // Fallback to INFO if invalid
        }
    }
}
