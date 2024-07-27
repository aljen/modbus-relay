#pragma once

#include "types.h"

modbus_rtu_error error_code_to_modbus_error(int32_t error_code);

MBAP deserialize_mbap_from_view(const data_view_t &data);

void serialize_mbap_to_view(const MBAP &mbap, data_view_t &data);

PDU deserialize_pdu_from_view(const data_view_t &data);

void debug_print_mbap(const MBAP &mbap);

void debug_print_pdu(const PDU &pdu);
