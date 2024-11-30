use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StopBits {
    One,
    Two,
}

impl From<StopBits> for serialport::StopBits {
    fn from(stop_bits: StopBits) -> Self {
        match stop_bits {
            StopBits::One => serialport::StopBits::One,
            StopBits::Two => serialport::StopBits::Two,
        }
    }
}

impl Default for StopBits {
    fn default() -> Self {
        Self::One
    }
}

impl std::fmt::Display for StopBits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopBits::One => write!(f, "1"),
            StopBits::Two => write!(f, "2"),
        }
    }
}
