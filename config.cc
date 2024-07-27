#include <fstream>
#include <iostream>
#include <filesystem>

#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>

#include "config.h"

using json = nlohmann::json;

Config g_config = generate_default_config();

Config& get_config()
{
    return g_config;
}

Config generate_default_config()
{
    Config config{};

    config.tcp_ip = "0.0.0.0";
    config.tcp_port = 5000;

    config.rtu_device = "/dev/ttyAMA0";
    config.rtu_serial_type = serial_type::RS485;
    config.rtu_baud_rate = 9600;
    config.rtu_parity = parity::None;
    config.rtu_data_bits = data_bits::Eight;
    config.rtu_stop_bit = stop_bit::One;
    config.rtu_slave_id = 0x0A;

    config.rtu_rts_enabled = true;
    config.rtu_rts_type = rts::Up;
    config.rtu_rts_delay = 3500;
    config.rtu_rts_manual_control = true;
    config.rtu_flush_after_write = true;

    config.rtu_enable_libmodbus_debug = false;

    return config;
}

void dump_default_config_to_output()
{
    Config config = generate_default_config();

    json data{};

    data["tcp_ip"] = config.tcp_ip;
    data["tcp_port"] = config.tcp_port;

    data["rtu_device"] = config.rtu_device;
    data["rtu_serial_type"] = config.rtu_serial_type == serial_type::RS485 ? "RS485" : "RS232";
    data["rtu_baud_rate"] = config.rtu_baud_rate;
    data["rtu_parity"] = [&] {
       switch (config.rtu_parity) {
           case parity::Even:
               return "Even";
           case parity::Odd:
               return "Odd";
           case parity::None:
           default:
               return "None";
       }
    }();
    data["rtu_data_bits"] = config.rtu_data_bits;
    data["rtu_stop_bit"] = config.rtu_stop_bit;
    data["rtu_slave_id"] = config.rtu_slave_id;

    data["rtu_rts_enabled"] = config.rtu_rts_enabled;
    data["rtu_rts_type"] = [&] {
       switch (config.rtu_rts_type) {
           case rts::Up:
               return "Up";
           case rts::Down:
               return "Down";
           case rts::None:
           default:
               return "None";
       }
    }();
    data["rtu_rts_delay"] = config.rtu_rts_delay;
    data["rtu_rts_manual_control"] = config.rtu_rts_manual_control;
    data["rtu_flush_after_write"] = config.rtu_flush_after_write;

    data["rtu_enable_libmodbus_debug"] = config.rtu_enable_libmodbus_debug;

    std::cout << std::setw(4) << data << std::endl;
}

bool config_from_json(Config &config, const json &data)
{
    config.tcp_ip = data["tcp_ip"].get<std::string>();
    config.tcp_port = data["tcp_port"].get<uint16_t>();

    config.rtu_device = data["rtu_device"].get<std::string>();

    const auto rtu_serial_type = data["rtu_serial_type"].get<std::string>();
    config.rtu_serial_type = [&] {
        if (rtu_serial_type == "RS485") {
            return serial_type::RS485;
        } else if (rtu_serial_type == "RS232") {
            return serial_type::RS232;
        } else {
            spdlog::error(R"(Invalid value for "rtu_serial_type": "{}")", rtu_serial_type);
            spdlog::error(R"(Valid values: "RS485" | "RS232")");
            abort();
        }
    }();

    config.rtu_baud_rate = data["rtu_baud_rate"].get<uint16_t>();

    const auto rtu_parity = data["rtu_parity"].get<std::string>();
    config.rtu_parity = [&] {
        if (rtu_parity == "None") {
            return parity::None;
        } else if (rtu_parity == "Even") {
            return parity::Even;
        } else if (rtu_parity == "Odd") {
            return parity::Odd;
        } else {
            spdlog::error(R"(Invalid value for "rtu_parity": "{}")", rtu_parity);
            spdlog::error(R"(Valid values: "None" | "Even" | "Odd")");
            abort();
        }
    }();

    const auto rtu_data_bits = data["rtu_data_bits"].get<uint16_t>();
    config.rtu_data_bits = [&] {
        if (rtu_data_bits < 5 || rtu_data_bits > 8) {
            spdlog::error(R"(Invalid value for "rtu_data_bits": "{}")", rtu_data_bits);
            spdlog::error("Valid values: 5 | 6 | 7 | 8");
            abort();
        }

        return static_cast<data_bits>(rtu_data_bits);
    }();

    const auto rtu_stop_bit = data["rtu_stop_bit"].get<uint8_t>();
    config.rtu_stop_bit = [&] {
        if (rtu_stop_bit != 1 && rtu_stop_bit != 2) {
            spdlog::error(R"(Invalid value for "rtu_stop_bit": "{}")", rtu_stop_bit);
            spdlog::error("Valid values: 1 | 2");
            abort();
        }

        return static_cast<stop_bit>(rtu_stop_bit);
    }();

    config.rtu_slave_id = data["rtu_slave_id"].get<uint8_t>();

    config.rtu_rts_enabled = data["rtu_rts_enabled"].get<bool>();

    const auto rtu_rts_type = data["rtu_rts_type"].get<std::string>();
    config.rtu_rts_type = [&] {
        if (rtu_rts_type == "None") {
            return rts::None;
        } else if (rtu_rts_type == "Up") {
            return rts::Up;
        } else if (rtu_rts_type == "Down") {
            return rts::Down;
        } else {
            spdlog::error(R"(Invalid value for "rtu_rts_type": "{}")", rtu_rts_type);
            spdlog::error(R"(Valid values: "None" | "Up" | "Down")");
            abort();
        }
    }();

    config.rtu_rts_delay = data["rtu_rts_delay"].get<uint16_t>();
    config.rtu_rts_manual_control = data["rtu_rts_manual_control"].get<bool>();
    config.rtu_flush_after_write = data["rtu_flush_after_write"].get<bool>();

    config.rtu_enable_libmodbus_debug = data["rtu_enable_libmodbus_debug"].get<bool>();

    return true;
}

bool load_config_from_file(std::string_view path)
{
    std::filesystem::path file_path = path;

    if (!exists(file_path)) {
        spdlog::warn("Configuration file {} not found, using default values.", path);
        spdlog::warn("Consider running with:");
        spdlog::warn("modbus_relay -dump-default-config > {}", path);
        return false;
    }

    std::ifstream file(file_path);
    json data = json::parse(file);

    return config_from_json(g_config, data);
}