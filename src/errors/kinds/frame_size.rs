#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSizeKind {
    TooShort,
    TooLong,
    BufferOverflow,
}

impl std::fmt::Display for FrameSizeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "Frame too short"),
            Self::TooLong => write!(f, "Frame too long"),
            Self::BufferOverflow => write!(f, "Buffer overflow"),
        }
    }
}
