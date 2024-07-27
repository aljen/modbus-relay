#include <iostream>

#include <asio.hpp>
#include <spdlog/spdlog.h>

#include "config.h"
#include "modbus.h"
#include "types.h"
#include "utils.h"

int main(int argc, char **argv) {
    if (argc == 2 && std::string_view{argv[1]} == "-dump-default-config") {
        dump_default_config_to_output();
        return 0;
    }

    load_config_from_file("/etc/modbus_relay.json");

    const auto &config = get_config();

    auto modbus_ctx = init_modbus_rtu();

    if (!modbus_ctx) {
        spdlog::error("Can't create modbus context");
        return -1;
    }

    try {
        using asio::ip::tcp;

        asio::io_context io_ctx{};
        tcp::endpoint endpoint{tcp::v4(), config.tcp_port};

        tcp::acceptor acceptor{io_ctx, endpoint};

        spdlog::debug("[main loop]");

        data_t request_buffer{};
        data_t answer_buffer{};

        while (true) {
            spdlog::debug("[waiting for connection]");

            tcp::socket socket{io_ctx};
            acceptor.accept(socket);

            asio::error_code error;

            const auto read = socket.read_some(asio::buffer(request_buffer), error);
            assert(read >= sizeof(MBAP));

            const data_view_t mbap_request_view{request_buffer.begin(), request_buffer.begin() + MBAP_SIZE};

            MBAP mbap_request = deserialize_mbap_from_view(mbap_request_view);

            const data_view_t request_view{
                    request_buffer.begin() + MBAP_SIZE,
                    request_buffer.begin() + MBAP_SIZE + (read - MBAP_SIZE)};
            const auto opt_answer_view = handle_modbus_rtu(modbus_ctx, request_view, answer_buffer);

            const auto function_code = request_view[0];

            if (opt_answer_view) {
                const auto &answer_view = *opt_answer_view;

                auto mbap_answer = mbap_request;
                mbap_answer.length = answer_view.size() + 3; // size + unit_id + function_code + length

                data_view_t mbap_answer_view{request_buffer.begin(), request_buffer.begin() + MBAP_SIZE};
                serialize_mbap_to_view(mbap_answer, mbap_answer_view);

                data_view_t new_answer_view{request_buffer.begin() + MBAP_SIZE,
                                            request_buffer.begin() + MBAP_SIZE - (request_buffer.size() - MBAP_SIZE)};

                new_answer_view[0] = function_code;
                new_answer_view[1] = answer_view.size();

                if (function_code == 3 || function_code == 4) {
                    for (size_t i = 0; i < answer_view.size() / 2; ++i) {
                        const auto index = i * 2;
                        new_answer_view[2 + index] = answer_view[index + 1];
                        new_answer_view[2 + index + 1] = answer_view[index];
                    }
                } else {
                    for (size_t i = 0; i < answer_view.size(); ++i) {
                        new_answer_view[2 + i] = answer_view[i];
                    }
                }

                const size_t adp_size = MBAP_SIZE + answer_view.size() + 2; // mbap + function_code + length + data
                asio::write(socket, asio::const_buffer(request_buffer.data(), adp_size), error);
            } else {
                assert(false && "Handle errors");
            }
        }
    }
    catch (std::exception &ex) {
        std::cerr << ex.what() << std::endl;
    }

    modbus_flush(modbus_ctx);
    modbus_free(modbus_ctx);

    return EXIT_SUCCESS;
}
