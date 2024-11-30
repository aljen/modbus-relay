#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemErrorKind {
    ResourceAllocation,
    PermissionDenied,
    FileSystem,
    Network,
    Other,
}

impl std::fmt::Display for SystemErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceAllocation => write!(f, "Resource allocation error"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::FileSystem => write!(f, "Filesystem error"),
            Self::Network => write!(f, "Network error"),
            Self::Other => write!(f, "Other system error"),
        }
    }
}
