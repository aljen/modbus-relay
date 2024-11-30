#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientErrorKind {
    ConnectionLost,
    Timeout,
    InvalidRequest,
    TooManyRequests,
    TooManyConnections,
    WriteError,
}

impl std::fmt::Display for ClientErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionLost => write!(f, "Connection lost"),
            Self::Timeout => write!(f, "Timeout"),
            Self::InvalidRequest => write!(f, "Invalid request"),
            Self::TooManyRequests => write!(f, "Too many requests"),
            Self::TooManyConnections => write!(f, "Too many connections"),
            Self::WriteError => write!(f, "Write error"),
        }
    }
}
