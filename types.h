#pragma once

#include <array>
#include <cstdint>
#include <expected>
#include <span>

constexpr const size_t MBAP_SIZE = 7;
constexpr const size_t PDU_SIZE = 5;

enum class modbus_rtu_error : uint8_t {
    Acknowledge,
    GatewayPathUnavailable,
    IllegalDataAddress,
    IllegalDataValue,
    IllegalFunction,
    InvalidCRC,
    InvalidData,
    InvalidExceptionCode,
    MemoryParityError,
    NegativeAcknowledge,
    ResponseNotFromRequestedSlave,
    SlaveDeviceOrServerFailure,
    SlaveDeviceOrServerIsBusy,
    TargetDeviceFailedToRespond,
    TooManyData,
};

enum class serial_type : uint8_t {
    RS232,
    RS485,
};

enum class parity : uint8_t {
    None = 'N',
    Even = 'E',
    Odd = 'O',
};

enum class data_bits : uint8_t {
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
};

enum class stop_bit : uint8_t {
    One = 1,
    Two = 2,
};

enum class rts : uint8_t {
    None,
    Up,
    Down,
};

using data_view_t = std::span<uint8_t>;
using data_result_t = std::expected<data_view_t, modbus_rtu_error>;
using data_t = std::array<uint8_t, 512>;

struct MBAP {
    uint16_t transaction_id;
    uint16_t protocol_id;
    uint16_t length;
    uint8_t unit_id;
};

struct PDU {
    uint8_t function_code;
    uint16_t start_address;
    uint16_t quantity;
};
