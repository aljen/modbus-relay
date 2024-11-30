pub mod config;
pub mod connection;
pub mod errors;
pub mod http_api;
pub mod modbus;
pub mod modbus_relay;
pub mod rtu_transport;
mod utils;

pub use config::{ConnectionConfig, HttpConfig, LoggingConfig, RelayConfig, RtuConfig, TcpConfig};
pub use config::{DataBits, Parity, RtsType, StopBits};
pub use connection::BackoffStrategy;
pub use connection::{ClientStats, ConnectionStats, IpStats};
pub use connection::{ConnectionGuard, ConnectionManager};
pub use errors::{
    BackoffError, ClientErrorKind, ConfigValidationError, ConnectionError, FrameErrorKind,
    IoOperation, ProtocolErrorKind, RelayError, RtsError, SerialErrorKind, TransportError,
};
pub use http_api::start_http_server;
pub use modbus::{guess_response_size, ModbusProcessor};
pub use modbus_relay::ModbusRelay;
pub use rtu_transport::RtuTransport;
