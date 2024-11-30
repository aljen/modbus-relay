#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOperation {
    Read,
    Write,
    Flush,
    Configure,
    Control,
    Listen,
}

impl std::fmt::Display for IoOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Flush => write!(f, "flush"),
            Self::Configure => write!(f, "configure"),
            Self::Control => write!(f, "control"),
            Self::Listen => write!(f, "listen"),
        }
    }
}
