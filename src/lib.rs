pub mod config;
pub mod connection_manager;
pub mod errors;
pub mod http_api;
pub mod modbus;
pub mod modbus_relay;
pub mod rtu_transport;
mod utils;

pub use config::RtsType;
pub use config::{Config as RelayConfig, HttpConfig, LoggingConfig, RtuConfig, TcpConfig};
pub use config::{DataBits, Parity, StopBits};
pub use errors::{
    BackoffError, ClientErrorKind, ConfigValidationError, ConnectionError, FrameErrorKind,
    IoOperation, ProtocolErrorKind, RelayError, RtsError, SerialErrorKind, TransportError,
};
pub use http_api::start_http_server;
pub use modbus::{guess_response_size, ModbusProcessor};
pub use modbus_relay::ModbusRelay;
pub use rtu_transport::RtuTransport;
