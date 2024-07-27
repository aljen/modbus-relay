#pragma once

#include <cstdint>
#include <string>

#include "types.h"

struct Config {
    std::string tcp_ip;
    uint16_t tcp_port;

    std::string rtu_device;
    serial_type rtu_serial_type;
    uint16_t rtu_baud_rate;
    parity rtu_parity;
    data_bits rtu_data_bits;
    stop_bit rtu_stop_bit;
    uint8_t rtu_slave_id;

    bool rtu_rts_enabled;
    rts rtu_rts_type;
    uint16_t rtu_rts_delay;
    bool rtu_rts_manual_control;
    bool rtu_flush_after_write;

    bool rtu_enable_libmodbus_debug;
};

Config& get_config();

Config generate_default_config();

bool load_config_from_file(std::string_view path);

void dump_default_config_to_output();
