use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Parity {
    None,
    Odd,
    Even,
}

impl From<Parity> for serialport::Parity {
    fn from(parity: Parity) -> Self {
        match parity {
            Parity::None => serialport::Parity::None,
            Parity::Odd => serialport::Parity::Odd,
            Parity::Even => serialport::Parity::Even,
        }
    }
}

impl Default for Parity {
    fn default() -> Self {
        Self::None
    }
}

impl std::fmt::Display for Parity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Parity::None => write!(f, "none"),
            Parity::Odd => write!(f, "odd"),
            Parity::Even => write!(f, "even"),
        }
    }
}
