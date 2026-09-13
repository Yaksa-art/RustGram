# ADR-0001: Single Rust Bridge via Plain C ABI Staticlib

- **Status:** Accepted
- **Date:** 2026-09-13
- **Phase:** 0 (Foundation)
- **Deciders:** Yaksa-art, INE
- **Revisits:** ROADMAP.md Decision Gate 1 ("Bridge technology")

## Context

RustGram migrates Telegram Desktop to Rust module by module while the client
keeps building and running after every merge (ROADMAP.md, Guiding Invariant 1).
That requires a boundary where Rust code lives inside the Qt/C++ application
from Phase 0 onward.

ROADMAP.md named cxx-qt as the plan of record for this bridge. During Phase 0
recon we discovered a stronger precedent already inside this repository:
Telegram's own dependency pipeline (`Telegram/build/prepare/prepare.py`,
stage `tlottie`) builds a Rust crate (dkaraush/tlottie) with
`cargo rustc --lib --crate-type staticlib` and links the result into the C++
application behind a C header. The repo therefore already blesses
**cargo-built staticlib + C ABI** as a delivery mechanism for Rust code.

## Decision

1. **One bridge crate.** `rust/crates/rustgram-bridge` is the ONLY Rust crate
   whose symbols cross into C++. All future ported modules (storage, protocol,
   media, UI logic) become pure-Rust dependencies of this crate. C++ never
   links any other Rust crate directly. (Invariant 5 of ROADMAP.md.)
2. **Plain C ABI, not cxx-qt, for Phase 0.** The bridge exposes `extern "C"`
   functions declared in `rustgram_bridge.h`, built as a staticlib, linked
   into the `Telegram` CMake target. Rationale:
   - Zero new third-party dependencies: no cxx-qt, no corrosion, no codegen
     step. The build gains exactly one tool it already has (`cargo`, pinned
     to the same 1.96.1 toolchain `prepare.py` installs).
   - The pattern is proven in-tree (tlottie) rather than evaluated from docs.
   - The Phase-0 surface is strings-in/strings-out (fingerprint, self-test),
     which needs no Qt type bindings — cxx-qt would be pure overhead.
   - Reversible: if Phase 1+ needs Qt type bindings, cxx-qt (or a second
     bridge crate behind the same boundary) can be adopted then, with this
     ADR superseded explicitly — never silently.
3. **Safety contract** (enforced in code, see `lib.rs` header docs):
   exports are safe `extern "C"` functions; strings cross as owned pointers
   freed by `rustgram_string_free`; panics are caught and become null/-1
   returns, never unwinds through C++ frames.
4. **Parity with repo conventions:** the bridge ships a C header with the
   same license banner style as the rest of the tree, and the CMake glue
   (`Telegram/cmake/rust_bridge.cmake`) follows the existing
   `td_*.cmake` module shape (option gate + `target_link_libraries`).

## Consequences

- **Good:** smallest possible Phase-0 blast radius; builds on all three
  platforms with the pinned toolchain; CI cost is one `cargo test` job plus
  one staticlib build per platform.
- **Good:** forces the (pointer, length, free) ownership discipline from day
  one, before any real module traffic depends on it.
- **Bad:** no Qt types cross the boundary yet — UI-adjacent ports (Phase 5)
  will need a richer binding story; that is accepted as a later ADR.
- **Bad:** staticlib-per-config means Debug and Release each pay a full
  `cargo build`; mitigated by shared `CARGO_TARGET_DIR` caching in CI.
- **Neutral:** `RUSTGRAM_BRIDGE` CMake option defaults ON; packagers who
  cannot run cargo can set `-DRUSTGRAM_BRIDGE=OFF` and get a pure-C++ build
  (the `logs.cpp` call site is `#ifdef`-guarded).

## Alternatives considered

- **cxx-qt now:** richer (real Qt bindings), but adds a large new dependency
  and codegen step for a Phase-0 surface that needs none of it. Deferred,
  not rejected — likely revisited at the Phase 4/5 boundary.
- **corrosion:** CMake-native cargo integration, but unvetted in this tree
  versus the in-tree tlottie precedent. Revisit if hand-rolled
  `add_custom_command` glue proves brittle.
- **Dynamic library (.dll/.so):** rejected — deployment story (portable
  builds, auto-updater signature checks in `generate_update_keys`) assumes
  a fixed artifact set; a staticlib changes nothing downstream.
