#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialErrorKind {
    OpenFailed,
    ReadFailed,
    WriteFailed,
    ConfigurationFailed,
    Disconnected,
    BufferOverflow,
    ParityError,
    FramingError,
}

impl std::fmt::Display for SerialErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpenFailed => write!(f, "Failed to open port"),
            Self::ReadFailed => write!(f, "Failed to read from port"),
            Self::WriteFailed => write!(f, "Failed to write to port"),
            Self::ConfigurationFailed => write!(f, "Failed to configure port"),
            Self::Disconnected => write!(f, "Port disconnected"),
            Self::BufferOverflow => write!(f, "Buffer overflow"),
            Self::ParityError => write!(f, "Parity error"),
            Self::FramingError => write!(f, "Framing error"),
        }
    }
}
