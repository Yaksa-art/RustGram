# This file is part of RustGram, derivative of Telegram Desktop,
# the official desktop application for the Telegram messaging service.
#
# For license and copyright information please follow this link:
# https://github.com/Yaksa-art/RustGram/blob/dev/LEGAL

# RustGram Phase 1 M6: Rust TL codegen driver (OPT-IN / EXPERIMENTAL).
#
# This module mirrors Telegram/cmake/generate_scheme.cmake but invokes the
# Rust port (rust/crates/tl-codegen, CLI: tl-codegen --config <json>
# -o<stem> <files...>) instead of Python
# (Telegram/SourceFiles/codegen/scheme/codegen_scheme.py via
# lib_tl/tl/generate_tl.py). It is NOT wired into any target yet:
# td_scheme.cmake keeps calling generate_scheme() until the cutover gate
# passes. generate_scheme.cmake is left untouched on purpose.
#
# Swap-in plan: once CI proves golden byte-identical output on api.tl +
# mtproto.tl (see docs/adr/ADR-0002-tl-codegen-cutover.md), replace the
# generate_scheme() call site(s) with rust_generate_scheme() one line at a
# time. Rollback is the reverse one-line edit.
#
# Notes on the mirror:
# - Same gen layout as generate_scheme(): ${CMAKE_CURRENT_BINARY_DIR}/gen
#   holding scheme.h/.cpp, scheme-dump_to_text.h/.cpp, and scheme.timestamp
#   (the Rust driver writes the literal "1" into the timestamp, like the
#   Python generator does; CMake only uses it as a custom-command stamp).
#   Conversion byproducts (-conversion-*.h/.cpp) are NOT listed: the
#   current codegen_scheme.json enables no conversion section, so the
#   driver emits exactly the same four files. If conversion is ever
#   enabled, extend gen_files here to match.
# - submodules_loc (used by generate_scheme() for the lib_tl Python path)
#   has no equivalent here: the generator lives in-tree at rust/, located
#   via CMAKE_SOURCE_DIR exactly like cmake/rust_bridge.cmake does
#   (repo root adds Telegram/ as a subdirectory, so CMAKE_SOURCE_DIR is
#   the repo root in both contexts).
# - The generator is invoked with `cargo run -p tl-codegen`, so cargo
#   owns the binary rebuild check. The GLOB below covers the crate
#   sources so CMake re-runs codegen when the generator itself changes
#   during Phase 1 development (new files need a re-configure to join
#   the glob — accepted, same caveat as every CMake GLOB).
#
# Usage (after cutover approval only):
#   include(cmake/rust_tl_codegen.cmake)
#   rust_generate_scheme(td_scheme "${scheme_files}")

function(rust_generate_scheme target_name script scheme_files)
    # `script` is accepted for drop-in compatibility with generate_scheme()
    # (td_scheme.cmake call site stays one-line-swappable); the Rust driver
    # uses codegen_scheme.json instead, so the Python script path is ignored.
    find_program(RUST_TL_CODEGEN_CARGO cargo
        DOC "Rust toolchain driver (pinned via rust/rust-toolchain.toml).")
    if (NOT RUST_TL_CODEGEN_CARGO)
        message(FATAL_ERROR
            "rust_generate_scheme: cargo not found, cannot build tl-codegen. "
            "Install the Rust toolchain (see rust/rust-toolchain.toml) or keep "
            "using generate_scheme() for a pure-C++/Python build.")
    endif()

    set(gen_dst ${CMAKE_CURRENT_BINARY_DIR}/gen)
    file(MAKE_DIRECTORY ${gen_dst})

    set(gen_timestamp ${gen_dst}/scheme.timestamp)
    set(gen_files
        ${gen_dst}/scheme.cpp
        ${gen_dst}/scheme.h
        ${gen_dst}/scheme-dump_to_text.cpp
        ${gen_dst}/scheme-dump_to_text.h
    )

    get_filename_component(rustgram_rust_dir ${CMAKE_SOURCE_DIR}/rust REALPATH)
    set(rust_tl_codegen_config ${rustgram_rust_dir}/crates/tl-codegen/codegen_scheme.json)
    set(rust_tl_codegen_manifest ${rustgram_rust_dir}/Cargo.toml)
    file(GLOB_RECURSE rust_tl_codegen_srcs
        ${rustgram_rust_dir}/crates/tl-codegen/src/*.rs
        ${rustgram_rust_dir}/crates/tl-codegen/Cargo.toml
    )

    add_custom_command(
    OUTPUT
        ${gen_timestamp}
    BYPRODUCTS
        ${gen_files}
    COMMAND
        ${CMAKE_COMMAND} -E env
        "CARGO_TARGET_DIR=${rustgram_rust_dir}/target"
        ${RUST_TL_CODEGEN_CARGO} run -q -p tl-codegen
        --manifest-path ${rust_tl_codegen_manifest}
        --
        --config ${rust_tl_codegen_config}
        -o${gen_dst}/scheme
        ${scheme_files}
    COMMENT "Generating scheme with Rust tl-codegen (${target_name})"
    DEPENDS
        ${rust_tl_codegen_config}
        ${rust_tl_codegen_srcs}
        ${scheme_files}
    VERBATIM
    )
    generate_target(${target_name} scheme ${gen_timestamp} "${gen_files}" ${gen_dst})
endfunction()
