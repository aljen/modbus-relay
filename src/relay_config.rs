use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RelayConfig {
    pub tcp_bind_addr: String,
    pub tcp_bind_port: u16,

    pub rtu_device: String,
    pub rtu_baud_rate: u32,

    pub transaction_timeout: Duration,

    #[cfg(feature = "rts")]
    pub rtu_rts_enabled: bool,
    #[cfg(feature = "rts")]
    pub rtu_rts_delay_ms: u64,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            tcp_bind_addr: "0.0.0.0".to_string(),
            tcp_bind_port: 502,

            rtu_device: "/dev/ttyAMA0".to_string(),
            rtu_baud_rate: 9600,

            transaction_timeout: Duration::from_secs(1),

            #[cfg(feature = "rts")]
            rtu_rts_enabled: true,
            #[cfg(feature = "rts")]
            rtu_rts_delay_ms: 0,
        }
    }
}

impl RelayConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.tcp_bind_port == 0 {
            return Err("TCP port cannot be 0".into());
        }

        if self.rtu_baud_rate == 0 {
            return Err("RTU baud rate cannot be 0".into());
        }

        if self.transaction_timeout.as_millis() == 0 {
            return Err("Transaction timeout cannot be 0".into());
        }

        #[cfg(feature = "rts")]
        if self.rtu_rts_enabled {
            // Maksymalny sensowny delay to powiedzmy 10 sekund
            if self.rtu_rts_delay_ms > 10000 {
                return Err("RTS delay too large (max 10000ms)".into());
            }
        }

        Ok(())
    }
}
