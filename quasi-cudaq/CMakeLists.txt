# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright 2026 QUASI Contributors
#
# Build system for the CUDA-Q → QUASI integration plugin.
#
# This builds libnvqir-quasi.so, which plugs into CUDA-Q's CircuitSimulator
# API and routes circuit execution through QUASI's Huoma simulator.
#
# Prerequisites:
#   - Rust toolchain (for quasi-cudaq-ffi)
#   - CUDA-Q installation (headers only — no GPU needed)
#   - CMake ≥ 3.18
#
# Build:
#   cd quasi-cudaq && mkdir build && cd build
#   cmake .. [-DCUDAQ_INCLUDE_DIR=/path/to/cudaq/include]
#   make -j$(nproc)
#
# The resulting libnvqir-quasi.so goes into CUDA-Q's plugin directory.

cmake_minimum_required(VERSION 3.18)
project(quasi-cudaq LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# ── Find CUDA-Q headers (optional — needed only for full plugin build) ───────

find_path(CUDAQ_INCLUDE_DIR "cudaq.h"
    PATHS
        /usr/local/include
        /opt/nvidia/cudaq/include
        $ENV{CUDAQ_INSTALL_PREFIX}/include
    PATH_SUFFIXES cudaq
    NO_DEFAULT_PATH
)

find_path(NVQIR_INCLUDE_DIR "CircuitSimulator.h"
    PATHS
        /usr/local/include
        /opt/nvidia/cudaq/include
        $ENV{CUDAQ_INSTALL_PREFIX}/include
    PATH_SUFFIXES nvqir
    NO_DEFAULT_PATH
)

# ── Build the Rust FFI library ───────────────────────────────────────────────

set(RUST_FFI_LIB ${CMAKE_BINARY_DIR}/libquasi_cudaq_ffi.a)
set(RUST_FFI_DYLIB ${CMAKE_BINARY_DIR}/libquasi_cudaq_ffi.so)

add_custom_command(
    OUTPUT ${RUST_FFI_LIB}
    COMMAND cargo build --release -p quasi-cudaq-ffi
    COMMAND ${CMAKE_COMMAND} -E copy
        ${CMAKE_SOURCE_DIR}/../target/release/libquasi_cudaq_ffi.a
        ${RUST_FFI_LIB}
    WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}/..
    COMMENT "Building quasi-cudaq-ffi Rust library"
    VERBATIM
)

add_custom_target(rust_ffi DEPENDS ${RUST_FFI_LIB})

# ── Build the CUDA-Q backend plugin ─────────────────────────────────────────

add_library(nvqir-quasi SHARED
    src/QuasiCircuitSimulator.cpp
    src/HamiltonianInterceptor.cpp
)

target_include_directories(nvqir-quasi PRIVATE
    ${CMAKE_SOURCE_DIR}/src
)

# Add CUDA-Q headers if found
if(CUDAQ_INCLUDE_DIR)
    target_include_directories(nvqir-quasi PRIVATE ${CUDAQ_INCLUDE_DIR})
    message(STATUS "Found CUDA-Q headers: ${CUDAQ_INCLUDE_DIR}")
endif()

if(NVQIR_INCLUDE_DIR)
    target_include_directories(nvqir-quasi PRIVATE ${NVQIR_INCLUDE_DIR})
    message(STATUS "Found NVQIR headers: ${NVQIR_INCLUDE_DIR}")
endif()

target_link_libraries(nvqir-quasi PRIVATE
    ${RUST_FFI_LIB}
    pthread
    dl
    m
)

add_dependencies(nvqir-quasi rust_ffi)

# ── Install ──────────────────────────────────────────────────────────────────

install(TARGETS nvqir-quasi
    LIBRARY DESTINATION lib
)

install(FILES
    src/HuomaFFI.h
    src/SolvayeurFFI.h
    src/QuasiCircuitSimulator.h
    DESTINATION include/quasi-cudaq
)
