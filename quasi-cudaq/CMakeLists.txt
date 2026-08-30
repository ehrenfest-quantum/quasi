# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright 2026 QUASI Contributors
#
# Build system for the CUDA-Q → QUASI integration plugin.
#
# Produces libnvqir-quasi.so, the NVQIR backend registered by
# NVQIR_REGISTER_SIMULATOR in src/QuasiCircuitSimulator.cpp. CUDA-Q's plugin
# loader resolves getCircuitSimulator_quasi() from it; the library name, the
# printed name in the macro, and QuasiCircuitSimulator::name() must all agree
# on "quasi".
#
# Build the plugin (requires a CUDA-Q installation):
#   cmake -S . -B build && cmake --build build -j
#
# Build only the CUDA-Q-independent unit tests:
#   cmake -S . -B build -DQUASI_CUDAQ_BUILD_PLUGIN=OFF
#   cmake --build build -j && ctest --test-dir build --output-on-failure

cmake_minimum_required(VERSION 3.24 FATAL_ERROR)
project(quasi-cudaq LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

option(QUASI_CUDAQ_BUILD_PLUGIN
    "Build the NVQIR backend plugin (requires a CUDA-Q installation)" ON)

include(CTest)

# ── Unit tests that do not need CUDA-Q ───────────────────────────────────────
#
# QuasiCircuitBuilder is deliberately free of CUDA-Q dependencies so gate
# recording and QASM emission can be tested without an installation.

add_executable(test_circuit_builder tests/test_circuit_builder.cpp)
target_include_directories(test_circuit_builder PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src)
target_compile_options(test_circuit_builder PRIVATE -Wall -Wextra)
add_test(NAME circuit_builder COMMAND test_circuit_builder)

if(NOT QUASI_CUDAQ_BUILD_PLUGIN)
    message(STATUS "QUASI: plugin build disabled — building unit tests only")
    return()
endif()

# ── CUDA-Q / NVQIR ───────────────────────────────────────────────────────────
#
# CUDA-Q ships NVQIRConfig.cmake, which provides nvqir_add_backend() and knows
# the include paths, link line, and install locations for backends. Requiring
# it is deliberate: a plugin built without NVQIR headers cannot subclass
# CircuitSimulatorBase and would not be loadable by CUDA-Q, so failing here is
# better than shipping a shared library that silently never registers.

find_package(NVQIR QUIET)

if(NOT NVQIR_FOUND)
    message(FATAL_ERROR
        "CUDA-Q (NVQIR) was not found.\n"
        "Set CUDAQ_INSTALL_PREFIX (or CMAKE_PREFIX_PATH) to your CUDA-Q "
        "installation, e.g.\n"
        "  cmake -S . -B build -DCMAKE_PREFIX_PATH=$CUDAQ_INSTALL_PREFIX\n"
        "To build only the CUDA-Q-independent unit tests instead, configure "
        "with -DQUASI_CUDAQ_BUILD_PLUGIN=OFF.")
endif()

# ── Rust FFI library ─────────────────────────────────────────────────────────

set(RUST_FFI_LIB ${CMAKE_CURRENT_BINARY_DIR}/libquasi_cudaq_ffi.a)

add_custom_command(
    OUTPUT ${RUST_FFI_LIB}
    COMMAND cargo build --release -p quasi-cudaq-ffi
    COMMAND ${CMAKE_COMMAND} -E copy
        ${CMAKE_CURRENT_SOURCE_DIR}/../target/release/libquasi_cudaq_ffi.a
        ${RUST_FFI_LIB}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/..
    COMMENT "Building quasi-cudaq-ffi Rust library"
    VERBATIM
)

add_custom_target(rust_ffi DEPENDS ${RUST_FFI_LIB})

# ── NVQIR backend plugin ─────────────────────────────────────────────────────
#
# nvqir_add_backend(<name> <gpu-requirements> <sources> <extra-libs>) creates
# the nvqir-<name> target with the naming and install conventions CUDA-Q
# expects.

nvqir_add_backend(quasi false
    "${CMAKE_CURRENT_SOURCE_DIR}/src/QuasiCircuitSimulator.cpp;${CMAKE_CURRENT_SOURCE_DIR}/src/HamiltonianInterceptor.cpp"
    "")

target_include_directories(nvqir-quasi PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src)

target_link_libraries(nvqir-quasi PRIVATE
    ${RUST_FFI_LIB}
    pthread
    dl
    m
)

add_dependencies(nvqir-quasi rust_ffi)

# ── Install ──────────────────────────────────────────────────────────────────
#
# nvqir_add_backend installs the library itself. The target config tells
# CUDA-Q how to select this backend; newer releases read it from targets/,
# older ones from platforms/.

install(FILES quasi.yml DESTINATION targets)

install(FILES
    src/HuomaFFI.h
    src/SolvayeurFFI.h
    src/QuasiCircuitBuilder.h
    src/QuasiCircuitSimulator.h
    DESTINATION include/quasi-cudaq
)
