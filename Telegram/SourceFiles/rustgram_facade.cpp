/*
This file is part of RustGram, derivative of Telegram Desktop
(the official desktop application for the Telegram messaging service).

For license and copyright information please follow this link:
https://github.com/Yaksa-art/RustGram/blob/dev/LEGAL
*/
#include "rustgram_facade.h"

#ifdef RUSTGRAM_BRIDGE_ENABLED

#include "logs.h"
#include "rustgram_bridge.h"

#include <atomic>

namespace RustGramBridge {
namespace {

std::atomic<bool> LoggedOnce = false;

void logFingerprintImpl() {
	// 1. Prove the (pointer, length, free) contract before trusting it.
	// A broken bridge must be loud at startup, not silent in production.
	constexpr auto kProbe = "rustgram-phase0";
	const auto probeLen = rustgram_selftest_roundtrip(kProbe);
	if (probeLen < 0) {
		LOG(("RustGram bridge self-test FAILED: roundtrip error."));
		return;
	}

	// 2. Pull the fingerprint. Null means Rust panicked across the
	// boundary: fatal, log it as such.
	char *raw = rustgram_fingerprint();
	if (!raw) {
		LOG(("RustGram bridge FAILED: null fingerprint (Rust panic)."));
		return;
	}

	// 3. Convert while owned, then release. The fingerprint is ASCII.
	const auto fingerprint = QString::fromUtf8(raw);
	rustgram_string_free(raw);

	LOG(("RustGram bridge: %1").arg(fingerprint));
}

} // namespace

void logFingerprintOnce() {
	if (LoggedOnce.exchange(true)) {
		return;
	}
	logFingerprintImpl();
}

} // namespace RustGramBridge

#endif // RUSTGRAM_BRIDGE_ENABLED
