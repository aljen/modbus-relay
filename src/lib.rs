pub mod connection_manager;
pub mod errors;
pub mod http_api;
pub mod logging;
pub mod modbus;
pub mod modbus_relay;
pub mod relay_config;
pub mod rtu_transport;

pub use errors::{
    BackoffError, ClientErrorKind, ConfigValidationError, ConnectionError, FrameErrorKind,
    IoOperation, ProtocolErrorKind, RelayError, RtsError, SerialErrorKind, TransportError,
};
pub use http_api::start_http_server;
pub use logging::{generate_request_id, setup_logging};
pub use modbus::{guess_response_size, ModbusProcessor};
pub use modbus_relay::ModbusRelay;
pub use relay_config::{DataBits, Parity, RelayConfig, RtsType, StopBits};
pub use rtu_transport::RtuTransport;
