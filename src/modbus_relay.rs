use rmodbus::{server::ModbusFrame, ErrorKind as ModbusError, ModbusProto};
use std::sync::Arc;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info};

use crate::{
    relay_config::RelayConfig,
    rtu_transport::{RtuTransport, TransportError},
};

#[derive(Error, Debug)]
pub enum RelayError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),
    #[error("Modbus protocol error: {0}")]
    Protocol(#[from] ModbusError),
    #[error("Configuration error: {0}")]
    Config(String),
}

pub struct ModbusRelay {
    transport: Arc<RtuTransport>,
    config: RelayConfig,
}

impl ModbusRelay {
    pub fn new(config: RelayConfig) -> Result<Self, RelayError> {
        let transport = RtuTransport::new(
            &config.rtu_device,
            config.rtu_baud_rate,
            config.transaction_timeout,
            #[cfg(feature = "rts")]
            config.rtu_rts_delay_ms,
        )
        .map_err(RelayError::Transport)?;

        Ok(Self {
            transport: Arc::new(transport),
            config,
        })
    }

    pub async fn run(&self) -> Result<(), RelayError> {
        let addr = format!(
            "{}:{}",
            self.config.tcp_bind_addr, self.config.tcp_bind_port
        );
        let listener = TcpListener::bind(&addr).await?;
        info!("Listening on {}", addr);

        loop {
            let (socket, peer) = listener.accept().await?;
            info!("New connection from {}", peer);

            let transport = Arc::clone(&self.transport);
            tokio::spawn(async move {
                if let Err(e) = handle_client(socket, transport).await {
                    error!("Client error: {}", e);
                }
            });
        }
    }
}

async fn handle_client(
    mut socket: TcpStream,
    transport: Arc<RtuTransport>,
) -> Result<(), RelayError> {
    let (mut reader, mut writer) = socket.split();

    loop {
        let mut tcp_buf = vec![0u8; 256];
        let mut rtu_response = Vec::new();
        let mut tcp_response = Vec::new();

        // Read the TCP request
        let n = reader.read(&mut tcp_buf).await?;
        if n == 0 {
            info!("Client disconnected");
            break;
        }

        // Save transaction ID
        let transaction_id = [tcp_buf[0], tcp_buf[1]];

        // Convert TCP to RTU
        let mut frame = ModbusFrame::new(
            tcp_buf[6], // unit_id
            &tcp_buf[..n],
            ModbusProto::TcpUdp,
            &mut rtu_response,
        );

        if frame.parse().is_err() {
            error!("Failed to parse TCP frame");
            continue;
        }

        // Generate RTU request
        frame.proto = ModbusProto::Rtu;
        frame.finalize_response()?;

        // Send and read the RTU response
        let mut rtu_buf = vec![0u8; 256];
        let rtu_len = transport.transaction(&rtu_response, &mut rtu_buf).await?;

        // Convert RTU response to TCP
        let mut tcp_frame = ModbusFrame::new(
            tcp_buf[6], // zachowujemy unit_id
            &rtu_buf[..rtu_len],
            ModbusProto::Rtu,
            &mut tcp_response,
        );

        tcp_frame.proto = ModbusProto::TcpUdp;
        tcp_frame.finalize_response()?;

        // Now we can safely modify the transaction ID
        tcp_response[0] = transaction_id[0];
        tcp_response[1] = transaction_id[1];

        // Write the TCP response
        writer.write_all(&tcp_response).await?;
    }

    Ok(())
}
