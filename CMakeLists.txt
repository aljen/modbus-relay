cmake_minimum_required(VERSION 3.28)
project(modbus_relay)

set(CMAKE_CXX_STANDARD 23)

find_package(fmt REQUIRED)
find_package(spdlog REQUIRED)
find_package(nlohmann_json REQUIRED)

include(FindPkgConfig)
pkg_check_modules(FMT REQUIRED fmt)
pkg_check_modules(SPDLOG REQUIRED spdlog)
pkg_check_modules(NJSON REQUIRED nlohmann_json)
pkg_check_modules(ASIO REQUIRED asio)
pkg_check_modules(MODBUS REQUIRED libmodbus)

add_executable(modbus_relay
        config.cc
        config.h
        main.cc
        modbus.cc
        modbus.h
        types.h
        utils.cc
        utils.h
)

target_include_directories(modbus_relay
    SYSTEM PRIVATE
        ${ASIO_INCLUDE_DIRS}
        ${FMT_INCLUDE_DIRS}
        ${MODBUS_INCLUDE_DIRS}
        ${NJSON_INCLUDE_DIRS}
        ${SPDLOG_INCLUDE_DIRS}
)
target_link_libraries(modbus_relay
    PRIVATE
        ${ASIO_LINK_LIBRARIES}
        ${FMT_LINK_LIBRARIES}
        ${MODBUS_LINK_LIBRARIES}
        ${NJSON_LINK_LIBRARIES}
        ${SPDLOG_LINK_LIBRARIES}
)
