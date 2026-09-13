/*
This file is part of RustGram, derivative of Telegram Desktop
(the official desktop application for the Telegram messaging service).

For license and copyright information please follow this link:
https://github.com/Yaksa-art/RustGram/blob/dev/LEGAL
*/
#pragma once

#include <cstddef>

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

// Returns the Rust bridge build fingerprint ("rustgram-bridge <ver> /
// <rustc> / <profile>") as a NUL-terminated UTF-8 string. The caller takes
// ownership and MUST release it with rustgram_string_free(). Returns null
// only if Rust panicked: treat as fatal bridge failure, not empty data.
// Thread-safe.
char *rustgram_fingerprint();

// Releases a string previously returned by this bridge (e.g.
// rustgram_fingerprint()). Passing null is a no-op. Passing a pointer
// from any other source is undefined behavior.
void rustgram_string_free(char *ptr);

// Copies up to out_len - 1 bytes of the fingerprint into out, plus NUL.
// Returns bytes written (excluding NUL), or -1 on null buffer, zero
// length, or internal panic. No heap transfer: nothing to free.
// Thread-safe.
long long rustgram_fingerprint_into(unsigned char *out, size_t out_len);

// Contract self-test: copies a NUL-terminated input through Rust and
// returns its byte length, or -1 on null input, invalid UTF-8, interior
// NUL, or panic. Called once at startup before any real bridge traffic.
long long rustgram_selftest_roundtrip(const char *input);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus
