<div align="center">

# modbus-relay

🚀 High-performance Modbus TCP to RTU relay written in Rust

[![Crates.io](https://img.shields.io/crates/v/modbus-relay.svg)](https://crates.io/crates/modbus-relay)
[![Documentation](https://docs.rs/modbus-relay/badge.svg)](https://docs.rs/modbus-relay)
[![License:MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://github.com/yourusername/modbus-relay/workflows/CI/badge.svg)](https://github.com/yourusername/modbus-relay/actions)

[Features](#features) •
[Installation](#installation) •
[Usage](#usage) •
[Configuration](#configuration) •
[Contributing](#contributing)

</div>

## 🌟 Features

- 🔄 Transparent TCP to RTU protocol conversion
- 🚀 Asynchronous I/O with Tokio
- 🔧 Optional RTS control for RS485 devices
- 🛡️ Thread-safe with proper error handling
- ⚡ Zero-copy buffer handling
- 📝 Comprehensive logging
- 🔌 Multiple concurrent TCP connections support

## 🚀 Quick Start

### Installation

```bash
# Install from crates.io
cargo install modbus-relay

# Or build from source
git clone https://github.com/yourusername/modbus-relay
cd modbus-relay
cargo build --release
```

### Basic Usage

```bash
# Generate default configuration
modbus-relay --dump-default-config > /etc/modbus-relay.json

# Run with custom config
modbus-relay -c /path/to/config.json

# Run with default settings
modbus-relay
```

## ⚙️ Configuration

Configuration file (`/etc/modbus-relay.json`):

```json
{
  "tcp_bind_addr": "0.0.0.0",
  "tcp_bind_port": 502,
  "rtu_device": "/dev/ttyAMA0",
  "rtu_baud_rate": 9600,
  "transaction_timeout": 1000
}
```

### RTS Support

Enable RTS control by building with the `rts` feature:

```bash
cargo build --features rts
```

Additional configuration options with RTS enabled:
```json
{
  "rtu_rts_enabled": true
}
```

## 🔍 Examples

### Thessla Green Recuperator Demo

![modbus_relay.png](docs/modbus_relay.png)

Running on Raspberry Pi 3 Model B+ with Thessla Green recuperator connected via `/dev/ttyAMA0`.

## 🛠️ Tech Stack

- [tokio](https://tokio.rs) - Async runtime
- [rmodbus](https://docs.rs/rmodbus) - Modbus protocol implementation
- [serialport](https://docs.rs/serialport) - Serial port handling
- [tracing](https://docs.rs/tracing) - Application logging
- [clap](https://docs.rs/clap) - Command line argument parsing
- [serde](https://serde.rs) - Configuration serialization

## 📝 License

This project is MIT licensed. See the [LICENSE](LICENSE) file for details.

## 👥 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## 🤔 Why?

Originally developed to solve the problem of slow development on a Raspberry Pi 3B+ when working with a Modbus RTU device. This relay allows you to develop and test your Modbus applications on more powerful machines while communicating with the RTU device over TCP/IP.

## ✨ Acknowledgements

Special thanks to the authors of [rmodbus](https://github.com/alttch/rmodbus) for providing an excellent Modbus implementation in Rust.

---

<div align="center">
Made with ❤️ by <a href="https://github.com/aljen">aljen</a>
</div>
