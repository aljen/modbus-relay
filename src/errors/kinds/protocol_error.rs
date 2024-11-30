#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolErrorKind {
    InvalidFunction,
    InvalidDataAddress,
    InvalidDataValue,
    ServerFailure,
    Acknowledge,
    ServerBusy,
    GatewayPathUnavailable,
    GatewayTargetFailedToRespond,
    InvalidProtocolId,
    InvalidTransactionId,
    InvalidUnitId,
    InvalidPdu,
}

impl std::fmt::Display for ProtocolErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFunction => write!(f, "Invalid function code"),
            Self::InvalidDataAddress => write!(f, "Invalid data address"),
            Self::InvalidDataValue => write!(f, "Invalid data value"),
            Self::ServerFailure => write!(f, "Server device failure"),
            Self::Acknowledge => write!(f, "Acknowledge"),
            Self::ServerBusy => write!(f, "Server device busy"),
            Self::GatewayPathUnavailable => write!(f, "Gateway path unavailable"),
            Self::GatewayTargetFailedToRespond => {
                write!(f, "Gateway target device failed to respond")
            }
            Self::InvalidProtocolId => write!(f, "Invalid protocol ID"),
            Self::InvalidTransactionId => write!(f, "Invalid transaction ID"),
            Self::InvalidUnitId => write!(f, "Invalid unit ID"),
            Self::InvalidPdu => write!(f, "Invalid PDU format"),
        }
    }
}

impl ProtocolErrorKind {
    pub fn to_exception_code(&self) -> u8 {
        match self {
            Self::InvalidFunction => 0x01,
            Self::InvalidDataAddress => 0x02,
            Self::InvalidDataValue => 0x03,
            Self::ServerFailure => 0x04,
            Self::Acknowledge => 0x05,
            Self::ServerBusy => 0x06,
            Self::GatewayPathUnavailable => 0x0A,
            Self::GatewayTargetFailedToRespond => 0x0B,
            _ => 0x04, // Map unknown errors to server failure
        }
    }

    pub fn from_exception_code(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(Self::InvalidFunction),
            0x02 => Some(Self::InvalidDataAddress),
            0x03 => Some(Self::InvalidDataValue),
            0x04 => Some(Self::ServerFailure),
            0x05 => Some(Self::Acknowledge),
            0x06 => Some(Self::ServerBusy),
            0x0A => Some(Self::GatewayPathUnavailable),
            0x0B => Some(Self::GatewayTargetFailedToRespond),
            _ => None,
        }
    }
}
