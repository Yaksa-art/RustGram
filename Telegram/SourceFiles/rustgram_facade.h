/*
This file is part of RustGram, derivative of Telegram Desktop
(the official desktop application for the Telegram messaging service).

For license and copyright information please follow this link:
https://github.com/Yaksa-art/RustGram/blob/dev/LEGAL
*/
#pragma once

// C++ facade over the rustgram-bridge staticlib (see ADR-0001).
//
// Only this translation unit talks to the raw C ABI in
// rust/crates/rustgram-bridge/include/rustgram_bridge.h. The rest of the
// application calls RustGramBridge::logFingerprintOnce() and never sees
// a raw pointer. Compiled only when RUSTGRAM_BRIDGE_ENABLED is defined
// (set by Telegram/cmake/rust_bridge.cmake); without it this header
// declares nothing and the .cpp compiles to an empty TU.

#ifdef RUSTGRAM_BRIDGE_ENABLED

namespace RustGramBridge {

// Runs the bridge self-test, then logs the Rust build fingerprint exactly
// once per process. Safe to call repeatedly; subsequent calls are no-ops.
// Must be called after the log system is up (Logs::start finished opening
// the main log) so the fingerprint lands in the log file.
void logFingerprintOnce();

} // namespace RustGramBridge

#endif // RUSTGRAM_BRIDGE_ENABLED
