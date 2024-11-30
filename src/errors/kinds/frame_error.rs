#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameErrorKind {
    TooShort,
    TooLong,
    InvalidFormat,
    InvalidUnitId,
    InvalidHeader,
    InvalidCrc,
    UnexpectedResponse,
}

impl std::fmt::Display for FrameErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "Frame too short"),
            Self::TooLong => write!(f, "Frame too long"),
            Self::InvalidFormat => write!(f, "Invalid frame format"),
            Self::InvalidUnitId => write!(f, "Invalid unit ID"),
            Self::InvalidHeader => write!(f, "Invalid frame header"),
            Self::InvalidCrc => write!(f, "Invalid frame CRC"),
            Self::UnexpectedResponse => write!(f, "Unexpected response"),
        }
    }
}
