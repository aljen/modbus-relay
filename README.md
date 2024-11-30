<div align="center">

# modbus-relay

🚀 High-performance Modbus TCP to RTU relay written in Rust

[![Crates.io](https://img.shields.io/crates/v/modbus-relay.svg)](https://crates.io/crates/modbus-relay)
[![Documentation](https://docs.rs/modbus-relay/badge.svg)](https://docs.rs/modbus-relay)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/aljen/modbus-relay/workflows/CI/badge.svg)](https://github.com/aljen/modbus-relay/actions)

[Features](#features) •
[Installation](#installation) •
[Usage](#usage) •
[Configuration](#configuration) •
[Monitoring](#monitoring) •
[Contributing](#contributing)

</div>

## 🌟 Features

- 🔄 Transparent TCP to RTU protocol conversion
- 🚀 High-performance asynchronous I/O with Tokio
- 🔧 Advanced RS485 support with configurable RTS control
- 🛡️ Robust error handling and connection management
- ⚡ Zero-copy buffer handling for optimal performance
- 📝 Structured logging with multiple output formats
- 🔌 Connection pooling with per-IP limits
- 🔄 Automatic reconnection with configurable backoff
- 🎯 Comprehensive test suite
- 📊 Built-in metrics and monitoring via HTTP API

## 🚀 Quick Start

### Installation

```bash
# Install from crates.io
cargo install modbus-relay

# Or build from source
git clone https://github.com/aljen/modbus-relay
cd modbus-relay
cargo build --release
```

### Basic Usage

```bash
# Generate default configuration
modbus-relay --dump-default-config > config.yaml

# Run with custom config
modbus-relay -c /path/to/config.yaml

# Run with default settings
modbus-relay
```

## ⚙️ Configuration

Configuration is managed through YAML files. Here's a complete example (`config.yaml`):

```yaml
tcp:
  bind_addr: "0.0.0.0"
  bind_port: 502

rtu:
  device: "/dev/ttyUSB0"
  baud_rate: 9600
  data_bits: 8
  parity: "none"
  stop_bits: 1
  flush_after_write: true
  rts_type: "none"  # Options: none, up, down
  rts_delay_us: 0
  transaction_timeout: "1s"
  serial_timeout: "100ms"
  max_frame_size: 256

http:
  enabled: true
  bind_addr: "127.0.0.1"
  bind_port: 8080
  metrics_enabled: true

connection:
  max_connections: 100
  idle_timeout: "60s"
  connect_timeout: "5s"
  per_ip_limits: 10
  backoff:
    initial_interval: "100ms"
    max_interval: "30s"
    multiplier: 2.0
    max_retries: 5

logging:
  trace_frames: false
  log_level: "info"
  format: "pretty"  # Options: pretty, json
  include_location: false
```

## 📊 Monitoring

The HTTP API provides basic monitoring endpoints:

- `GET /health` - Health check endpoint
- `GET /status` - Detailed status information

Planned monitoring features:
- Prometheus metrics support
- OpenTelemetry integration
- Advanced connection statistics
- Detailed performance metrics

## 🔍 Examples

### Industrial Automation Setup

![modbus_relay.png](docs/modbus_relay.png)

Example setup running on Raspberry Pi with multiple Modbus RTU devices connected via RS485.

## 🛠️ Tech Stack

- [tokio](https://tokio.rs) - Asynchronous runtime
- [tokio-serial](https://docs.rs/tokio-serial) - Async serial port handling
- [tracing](https://docs.rs/tracing) - Structured logging
- [config](https://docs.rs/config) - Configuration management
- [axum](https://docs.rs/axum) - HTTP server framework

### Coming Soon
- Prometheus metrics integration
- OpenTelemetry support

## 📚 Documentation

- [API Documentation](https://docs.rs/modbus-relay)
- [Configuration Guide](docs/configuration.md)
- [Metrics Reference](docs/metrics.md)
- [Troubleshooting Guide](docs/troubleshooting.md)

## 🤝 Contributing

Contributions are welcome! Please check out our:
- [Contributing Guidelines](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
