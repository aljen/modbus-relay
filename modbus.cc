#include <cassert>
#include <mutex>
#include <optional>
#include <sys/ioctl.h>

#include <spdlog/spdlog.h>

#include "config.h"
#include "modbus.h"
#include "utils.h"

std::mutex g_mutex{};

modbus_t *init_modbus_rtu() {
    const auto config = get_config();

    const auto rtu_device = config.rtu_device;
    const auto rtu_serial_type = config.rtu_serial_type == serial_type::RS485 ? MODBUS_RTU_RS485 : MODBUS_RTU_RS232;
    const auto rtu_baud_rate = config.rtu_baud_rate;
    const auto rtu_parity = static_cast<char>(config.rtu_parity);
    const auto rtu_data_bits = static_cast<uint8_t>(config.rtu_data_bits);
    const auto rtu_stop_bit = static_cast<uint8_t>(config.rtu_stop_bit);
    const auto rtu_slave_id = config.rtu_slave_id;

    const auto rtu_rts_enabled = config.rtu_rts_enabled;
    const auto rtu_rts_type = static_cast<uint8_t>(config.rtu_rts_type);
    const auto rtu_rts_delay = config.rtu_rts_delay;
    const auto rtu_use_custom_rts = config.rtu_rts_manual_control || config.rtu_flush_after_write;

    spdlog::info("Connecting to {}", rtu_device);

    spdlog::debug("calling modbus_new_rtu");
    spdlog::debug( "rtu_device: {}", rtu_device.data());
    modbus_t *modbus_ctx = modbus_new_rtu(rtu_device.data(), rtu_baud_rate, rtu_parity, rtu_data_bits, rtu_stop_bit);

    spdlog::debug("calling modbus_set_debug");
    modbus_set_debug(modbus_ctx, config.rtu_enable_libmodbus_debug);

    modbus_set_slave(modbus_ctx, rtu_slave_id);

    modbus_rtu_set_serial_mode(modbus_ctx, rtu_serial_type);

    if (rtu_rts_enabled) {
        modbus_rtu_set_rts(modbus_ctx, rtu_rts_type);
        modbus_rtu_set_rts_delay(modbus_ctx, rtu_rts_delay);
        if (rtu_use_custom_rts) {
            modbus_rtu_set_custom_rts(modbus_ctx, handle_modbus_rtu_rts);
        }
    }

    if (modbus_connect(modbus_ctx) == -1) {
        spdlog::error("{}: {}\n", rtu_device, modbus_strerror(errno));
        modbus_free(modbus_ctx);
        return nullptr;
    }

    modbus_flush(modbus_ctx);

    spdlog::info("Connected to {}", rtu_device);

    return modbus_ctx;
}

void handle_modbus_rtu_rts(modbus_t *ctx, int32_t on) {
    const auto config = get_config();

    if (config.rtu_rts_manual_control) {
        // TODO(aljen): tty_fd is second int in modbus_t struct, hacky, I know.
        const auto fd = *(reinterpret_cast<int *>(ctx) + 1);

        int32_t flags{};
        ioctl(fd, TIOCMGET, &flags);

        if (on) {
            flags |= TIOCM_RTS;
        } else {
            flags &= ~TIOCM_RTS;
        }

        ioctl(fd, TIOCMSET, &flags);
    }

    if (config.rtu_flush_after_write && on) {
        modbus_flush(ctx);
    }
}

data_result_t
handle_read_coil_status(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer) {
    assert(false && "Not implemented yet");
    return std::unexpected{modbus_rtu_error::IllegalFunction};
}

data_result_t
handle_read_input_status(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer) {
    assert(false && "Not implemented yet");
    return std::unexpected{modbus_rtu_error::IllegalFunction};
}

data_result_t
handle_read_holding_registers(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer) {
    assert(false && "Not implemented yet");
    return std::unexpected{modbus_rtu_error::IllegalFunction};
}

data_result_t
handle_read_input_registers(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer) {
    assert((answer_buffer.size() / 2) >= quantity);

    std::optional<modbus_rtu_error> error_code{};
    size_t registers_read{};

    {
        std::scoped_lock lock{g_mutex};

        auto regs_data = reinterpret_cast<uint16_t *>(answer_buffer.data());
        const auto rc = modbus_read_input_registers(modbus_ctx, start_address, quantity, regs_data);

        if (rc >= 0) {
            registers_read = rc;
        } else {
            error_code = error_code_to_modbus_error(errno);
        }
    }

    if (error_code) {
        spdlog::error("Error: {} ({})\n", modbus_strerror(errno), static_cast<uint32_t>(*error_code));
        return std::unexpected{*error_code};
    }

    return data_view_t{answer_buffer.begin(), answer_buffer.begin() + registers_read * 2};
}

data_result_t handle_force_single_coil() {
    assert(false && "Not implemented yet");
    return std::unexpected{modbus_rtu_error::IllegalFunction};
}

data_result_t handle_preset_single_register() {
    assert(false && "Not implemented yet");
    return std::unexpected{modbus_rtu_error::IllegalFunction};
}

data_result_t handle_force_multiple_coils() {
    assert(false && "Not implemented yet");
    return std::unexpected{modbus_rtu_error::IllegalFunction};
}

data_result_t handle_preset_multiple_registers() {
    assert(false && "Not implemented yet");
    return std::unexpected{modbus_rtu_error::IllegalFunction};
}

data_result_t handle_modbus_rtu(modbus_t *modbus_ctx, const data_view_t &request_view, data_t &answer_buffer) {
    switch (request_view[0]) {
        case 0x01: {
            assert(request_view.size() == PDU_SIZE);
            const auto pdu = deserialize_pdu_from_view(request_view);
            return handle_read_coil_status(modbus_ctx, pdu.start_address, pdu.quantity, answer_buffer);
        }
        case 0x02: {
            assert(request_view.size() == PDU_SIZE);
            const auto pdu = deserialize_pdu_from_view(request_view);
            return handle_read_input_status(modbus_ctx, pdu.start_address, pdu.quantity, answer_buffer);
        }
        case 0x03: {
            assert(request_view.size() == PDU_SIZE);
            const auto pdu = deserialize_pdu_from_view(request_view);
            return handle_read_holding_registers(modbus_ctx, pdu.start_address, pdu.quantity, answer_buffer);
        }
        case 0x04: {
            assert(request_view.size() == PDU_SIZE);
            const auto pdu = deserialize_pdu_from_view(request_view);
            return handle_read_input_registers(modbus_ctx, pdu.start_address, pdu.quantity, answer_buffer);
        }
        case 0x05: {
            return handle_force_single_coil();
        }
        case 0x06: {
            return handle_preset_single_register();
        }
        case 0x0F: {
            return handle_force_multiple_coils();
        }
        case 0x10: {
            return handle_preset_multiple_registers();
        }
        default:
            assert(false && "Invalid function code");
    }

    return std::unexpected{modbus_rtu_error::IllegalFunction};
}
