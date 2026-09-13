# This file is part of RustGram, derivative of Telegram Desktop,
# the official desktop application for the Telegram messaging service.
#
# For license and copyright information please follow this link:
# https://github.com/Yaksa-art/RustGram/blob/dev/LEGAL

# RustGram Phase 0: builds the rustgram-bridge staticlib with cargo and
# links it into the Telegram target. See ADR-0001
# (docs/adr/ADR-0001-single-rust-bridge.md) for why this is a plain C ABI
# staticlib (same delivery mechanism as Telegram's own tlottie dependency)
# rather than cxx-qt.
#
# Usage: include(cmake/rust_bridge.cmake) after src_loc is defined, then
# the Telegram target links tdesktop::rustgram_bridge.
#
# Option RUSTGRAM_BRIDGE (default ON): packagers without cargo can pass
# -DRUSTGRAM_BRIDGE=OFF for a pure-C++ build; the logs.cpp call site is
# ifdef-guarded on RUSTGRAM_BRIDGE_ENABLED.

option(RUSTGRAM_BRIDGE "Build and link the RustGram Rust bridge (requires cargo)." ON)

if (NOT RUSTGRAM_BRIDGE)
    message(STATUS "RustGram bridge disabled: pure-C++ build.")
    return()
endif()

find_program(RUSTGRAM_CARGO cargo REQUIRED
    DOC "Rust toolchain driver (pinned via rust/rust-toolchain.toml).")

get_filename_component(rustgram_rust_dir ${CMAKE_CURRENT_SOURCE_DIR}/../rust REALPATH)
set(rustgram_bridge_crate_dir ${rustgram_rust_dir}/crates/rustgram-bridge)
set(rustgram_bridge_include_dir ${rustgram_bridge_crate_dir}/include)

if (CMAKE_BUILD_TYPE STREQUAL "Debug" OR CMAKE_CONFIGURATION_TYPES MATCHES "Debug")
    set(rustgram_cargo_profile debug)
    set(rustgram_lib_dir ${rustgram_rust_dir}/target/debug)
else()
    set(rustgram_cargo_profile release)
    set(rustgram_lib_dir ${rustgram_rust_dir}/target/release)
endif()

if (WIN32)
    set(rustgram_bridge_lib ${rustgram_lib_dir}/rustgram_bridge.lib)
else()
    set(rustgram_bridge_lib ${rustgram_lib_dir}/librustgram_bridge.a)
endif()

add_custom_command(
    OUTPUT ${rustgram_bridge_lib}
    COMMAND ${CMAKE_COMMAND} -E env
        "CARGO_TARGET_DIR=${rustgram_rust_dir}/target"
        ${RUSTGRAM_CARGO} build -p rustgram-bridge
        --manifest-path ${rustgram_rust_dir}/Cargo.toml
        $<IF:$<CONFIG:Debug>,,--release>
    WORKING_DIRECTORY ${rustgram_rust_dir}
    DEPENDS
        ${rustgram_bridge_crate_dir}/Cargo.toml
        ${rustgram_bridge_crate_dir}/build.rs
        ${rustgram_bridge_crate_dir}/src/lib.rs
    COMMENT "Building rustgram-bridge staticlib with cargo (${rustgram_cargo_profile})."
    VERBATIM
)

add_custom_target(rustgram_bridge_lib DEPENDS ${rustgram_bridge_lib})

add_library(tdesktop_rustgram_bridge INTERFACE)
add_library(tdesktop::rustgram_bridge ALIAS tdesktop_rustgram_bridge)
add_dependencies(tdesktop_rustgram_bridge rustgram_bridge_lib)

target_include_directories(tdesktop_rustgram_bridge
INTERFACE
    ${rustgram_bridge_include_dir}
)

target_link_libraries(tdesktop_rustgram_bridge
INTERFACE
    ${rustgram_bridge_lib}
)

target_compile_definitions(tdesktop_rustgram_bridge
INTERFACE
    RUSTGRAM_BRIDGE_ENABLED
)
