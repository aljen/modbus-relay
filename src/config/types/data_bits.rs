use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DataBits(u8);

impl DataBits {
    pub fn new(bits: u8) -> Option<Self> {
        match bits {
            5..=8 => Some(Self(bits)),
            _ => None,
        }
    }

    pub fn get(&self) -> u8 {
        self.0
    }
}

impl Default for DataBits {
    fn default() -> Self {
        Self(8)
    }
}

impl From<DataBits> for serialport::DataBits {
    fn from(data_bits: DataBits) -> Self {
        match data_bits.0 {
            5 => serialport::DataBits::Five,
            6 => serialport::DataBits::Six,
            7 => serialport::DataBits::Seven,
            8 => serialport::DataBits::Eight,
            _ => unreachable!("DataBits constructor ensures valid values"),
        }
    }
}

impl std::fmt::Display for DataBits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
