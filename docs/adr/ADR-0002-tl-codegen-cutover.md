# ADR-0002: TL Codegen Cutover (Python → Rust)

- **Status:** Proposed (opt-in, experimental)
- **Date:** 2026-09-14
- **Phase:** 1 (M6)
- **Deciders:** Yaksa-art, INE
- **Supersedes:** nothing (complements ADR-0001; bridge decision untouched)

## Context

Phase 1 ports Telegram's TL codegen (`Telegram/SourceFiles/mtproto/scheme/*.tl`
via `codegen_scheme.py` + `lib_tl/tl/generate_tl.py`) to a Rust binary
(`rust/crates/tl-codegen`, CLI: `tl-codegen --config <json> -o<stem>
<files...>`). The build glue for the cutover is
`Telegram/cmake/rust_tl_codegen.cmake`, which mirrors
`Telegram/cmake/generate_scheme.cmake` but invokes the Rust binary via
`cargo run -p tl-codegen` instead of Python.

## Decision

1. **What gets replaced:** the single `generate_scheme(td_scheme ...)` call in
   `Telegram/cmake/td_scheme.cmake` — swapped to
   `rust_generate_scheme(td_scheme "${scheme_files}")` — once the golden gate
   passes. `generate_scheme.cmake` itself is never edited; the new module
   lives alongside it.
2. **Golden gate:** CI must prove byte-identical output on the real inputs
   (`api.tl` + `mtproto.tl`): `scheme.h`, `scheme.cpp`,
   `scheme-dump_to_text.h`, `scheme-dump_to_text.cpp`, each diffed against
   the Python-generated golden files. No whitespace or ordering drift
   allowed — byte-identical or no cutover.
3. **Rollback:** one-line revert — change the `rust_generate_scheme(...)`
   call site back to `generate_scheme(td_scheme <script>
   "${scheme_files}")`. No other file moves.
4. **Upstream freeze:** everything under `Telegram/` that came from upstream
   tdesktop (`generate_scheme.cmake`, `td_scheme.cmake` beyond the one call
   line, `lib_tl/`, the `.tl` schemes) stays byte-identical to upstream
   until the cutover commit. The new module and the Rust crate are the only
   Phase-1-owned surfaces.

## Consequences

- **Good:** build drops the Python codegen dependency once cut over; the
  generator becomes testable Rust with golden tests in-tree.
- **Good:** zero-risk trial period — the module ships unwired, so main
  builds are unaffected until the swap.
- **Bad:** `cargo run` adds a Rust build to scheme generation (cold CI pays
  it once; `CARGO_TARGET_DIR` caching mitigates).
- **Neutral:** `find_program(cargo)` fails hard in the new module (unlike
  the bridge's graceful OFF) — by the time anyone calls it, they opted in.
