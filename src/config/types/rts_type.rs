use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RtsType {
    /// RTS disabled
    None,
    /// RTS = High during transmission
    Up,
    /// RTS = LOW during transmission
    #[default]
    Down,
}

impl RtsType {
    pub fn to_signal_level(&self, is_transmitting: bool) -> bool {
        match self {
            RtsType::None => false,
            RtsType::Up => is_transmitting,
            RtsType::Down => !is_transmitting,
        }
    }
}

impl std::fmt::Display for RtsType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RtsType::None => write!(f, "none"),
            RtsType::Up => write!(f, "up"),
            RtsType::Down => write!(f, "down"),
        }
    }
}
