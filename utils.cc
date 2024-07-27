#include <cassert>
#include <cstdio>

#include <spdlog/spdlog.h>
#include <modbus.h>

#include "types.h"

modbus_rtu_error error_code_to_modbus_error(const int32_t error_code) {
    switch (error_code) {
        case EMBXILFUN:
            return modbus_rtu_error::IllegalFunction;
        case EMBXILADD:
            return modbus_rtu_error::IllegalDataAddress;
        case EMBXILVAL:
            return modbus_rtu_error::IllegalDataValue;
        case EMBXSFAIL:
            return modbus_rtu_error::SlaveDeviceOrServerFailure;
        case EMBXACK:
            return modbus_rtu_error::Acknowledge;
        case EMBXSBUSY:
            return modbus_rtu_error::SlaveDeviceOrServerIsBusy;
        case EMBXNACK:
            return modbus_rtu_error::NegativeAcknowledge;
        case EMBXMEMPAR:
            return modbus_rtu_error::MemoryParityError;
        case EMBXGPATH:
            return modbus_rtu_error::GatewayPathUnavailable;
        case EMBXGTAR:
            return modbus_rtu_error::TargetDeviceFailedToRespond;
        case EMBBADCRC:
            return modbus_rtu_error::InvalidCRC;
        case EMBBADDATA:
            return modbus_rtu_error::InvalidData;
        case EMBBADEXC:
            return modbus_rtu_error::InvalidExceptionCode;
        case EMBMDATA:
            return modbus_rtu_error::TooManyData;
        case EMBBADSLAVE:
            return modbus_rtu_error::ResponseNotFromRequestedSlave;
        default:
            spdlog::error("Invalid modbus error code: {}", error_code);
    }

    return modbus_rtu_error::InvalidExceptionCode;
}

MBAP deserialize_mbap_from_view(const data_view_t &data) {
    MBAP mbap{};

    mbap.transaction_id = data[0] << 8 | data[1];
    mbap.protocol_id = data[2] << 8 | data[3];
    mbap.length = data[4] << 8 | data[5];
    mbap.unit_id = data[6];

    return mbap;
}

void serialize_mbap_to_view(const MBAP &mbap, data_view_t &data) {
    data[0] = static_cast<uint8_t>((mbap.transaction_id >> 8) & 0x00FF);
    data[1] = static_cast<uint8_t>(mbap.transaction_id & 0x00FF);
    data[2] = static_cast<uint8_t>((mbap.protocol_id >> 8) & 0x00FF);
    data[3] = static_cast<uint8_t>(mbap.protocol_id & 0x00FF);
    data[4] = static_cast<uint8_t>((mbap.length >> 8) & 0x00FF);
    data[5] = static_cast<uint8_t>(mbap.length & 0x00FF);
    data[6] = mbap.unit_id;
}

PDU deserialize_pdu_from_view(const data_view_t &data) {
    PDU pdu{};

    pdu.function_code = data[0];
    pdu.start_address = data[1] << 8 | data[2];
    pdu.quantity = data[3] << 8 | data[4];

    return pdu;
}

void debug_print_mbap(const MBAP &mbap) {
    printf("MBAP:\n");
    printf(" transaction_id: %04X\n", mbap.transaction_id);
    printf("    protocol_id: %04X\n", mbap.protocol_id);
    printf("         length: %04X\n", mbap.length);
    printf("        unit_id: %02X\n", mbap.unit_id);
}

void debug_print_pdu(const PDU &pdu) {
    printf("PDU:\n");
    printf(" function_code: %02X\n", pdu.function_code);
    printf(" start_address: %04X\n", pdu.start_address);
    printf("      quantity: %04X\n", pdu.quantity);
}
