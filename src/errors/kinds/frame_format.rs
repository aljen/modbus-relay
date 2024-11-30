#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormatKind {
    InvalidHeader,
    InvalidFormat,
    UnexpectedResponse,
}

impl std::fmt::Display for FrameFormatKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "Invalid frame header"),
            Self::InvalidFormat => write!(f, "Invalid frame format"),
            Self::UnexpectedResponse => write!(f, "Unexpected response"),
        }
    }
}
