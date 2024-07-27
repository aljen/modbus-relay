#pragma once

#include <modbus.h>

#include "types.h"

modbus_t *init_modbus_rtu();

void handle_modbus_rtu_rts(modbus_t *ctx, int32_t on);

data_result_t
handle_read_coil_status(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer);

data_result_t
handle_read_input_status(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer);

data_result_t
handle_read_holding_registers(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer);

data_result_t
handle_read_input_registers(modbus_t *modbus_ctx, uint16_t start_address, uint16_t quantity, data_t &answer_buffer);

data_result_t handle_force_single_coil();

data_result_t handle_preset_single_register();

data_result_t handle_force_multiple_coils();

data_result_t handle_preset_multiple_registers();

data_result_t handle_modbus_rtu(modbus_t *modbus_ctx, const data_view_t &request_view, data_t &answer_buffer);
