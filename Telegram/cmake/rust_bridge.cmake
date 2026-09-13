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

# Cargo is REQUIRED on dev machines, but some packager environments (notably
# the Linux Docker image and the Snapcraft build env) have no Rust toolchain.
# Failing hard there would break platforms Phase 0 doesn't own yet, so the
# bridge degrades to OFF with a loud warning instead. The dedicated
# rustgram.yml workflow still proves the bridge on all three hosted OSes.
find_program(RUSTGRAM_CARGO cargo
    DOC "Rust toolchain driver (pinned via rust/rust-toolchain.toml).")
if (NOT RUSTGRAM_CARGO)
    message(WARNING
        "RustGram bridge: cargo not found, building WITHOUT Rust support. "
        "Install the Rust toolchain (see rust/rust-toolchain.toml) to enable it.")
    set(RUSTGRAM_BRIDGE OFF)
    return()
endif()

# CMAKE_SOURCE_DIR is the repo root (root CMakeLists adds Telegram/ as a
# subdirectory), so this survives regardless of which CMakeLists includes us.
get_filename_component(rustgram_rust_dir ${CMAKE_SOURCE_DIR}/rust REALPATH)
set(rustgram_bridge_crate_dir ${rustgram_rust_dir}/crates/rustgram-bridge)
set(rustgram_bridge_include_dir ${rustgram_bridge_crate_dir}/include)

# NOTE: the per-config library path cannot be a plain variable with
# multi-config generators (Visual Studio, Ninja Multi-Config — both used by
# win.yml): target/debug vs target/release is only known at build time.
# The library path below is therefore a generator expression.
if (WIN32)
    set(rustgram_bridge_lib_name rustgram_bridge.lib)
else()
    set(rustgram_bridge_lib_name librustgram_bridge.a)
endif()
set(rustgram_bridge_lib
    ${rustgram_rust_dir}/target/$<IF:$<CONFIG:Debug>,debug,release>/${rustgram_bridge_lib_name})

# NOTE: $<$<NOT:$<CONFIG:Debug>>:--release> (NOT $<IF:...>) — a false
# $<...> expands to ZERO arguments (removed), while a false $<IF:...>
# branch expands to ONE empty-string argument, which cargo rejects with
# "error: unexpected argument '' found" (broke win.yml Debug builds).

add_custom_command(
    OUTPUT ${rustgram_bridge_lib}
    COMMAND ${CMAKE_COMMAND} -E env
        "CARGO_TARGET_DIR=${rustgram_rust_dir}/target"
        ${RUSTGRAM_CARGO} build -p rustgram-bridge
        --manifest-path ${rustgram_rust_dir}/Cargo.toml
        $<$<NOT:$<CONFIG:Debug>>:--release>
    WORKING_DIRECTORY ${rustgram_rust_dir}
    DEPENDS
        ${rustgram_bridge_crate_dir}/Cargo.toml
        ${rustgram_bridge_crate_dir}/build.rs
        ${rustgram_bridge_crate_dir}/src/lib.rs
    COMMENT "Building rustgram-bridge staticlib with cargo."
    VERBATIM
)

add_custom_target(rustgram_bridge_lib DEPENDS ${rustgram_bridge_lib})
# The staticlib is produced by cargo, not by CMake: without this, parallel
# builds race Telegram's link step against the still-running cargo build.
# (Single-config Make generators may order it correctly by luck; Visual
# Studio and Ninja Multi-Config do not.)
add_dependencies(Telegram rustgram_bridge_lib)

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
