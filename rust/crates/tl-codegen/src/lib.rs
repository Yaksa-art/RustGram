//! `tl-codegen`: Rust port of `lib_tl/tl/generate_tl.py`.
//!
//! Turns `mtproto/scheme/*.tl` into C++ (`scheme.h/.cpp`,
//! `scheme-dump_to_text.h/.cpp`, and — when configured —
//! `scheme-conversion-*.h/.cpp`). The differential test
//! (`tests/golden.rs`) requires **byte-identical** output to the Python
//! generator on the real scheme; that is the exit criterion for Phase 1.
//!
//! Module map mirrors the Python for reviewability:
//! - [`config`] — the `generate({...})` dict as a struct.
//! - [`tl`] — `readInputs` + comment/tag predicates.
//! - [`crc`] — CRC-32 type-id verification.
//! - [`parse`] — constructor/param regexes + name helpers.
//! - [`ir`] — typed scheme representation (replaces 13-tuples).
//! - [`resolve`] — `fullTypeName` + `handleTemplate`.
//! - [`emit`] — C++ emitters (raw strings, no templates).
//! - [`driver`] — `readAndGenerate` orchestration + idempotent writes.

pub mod config;
pub mod crc;
pub mod driver;
pub mod emit;
pub mod ir;
pub mod parse;
pub mod parser;
pub mod resolve;
pub mod tl;

pub use config::CodegenScheme;
pub use driver::{read_and_generate, Outputs};
pub use ir::Scheme;

/// Parse stage (M1): full main-loop port in [`parser`].
pub fn parse_scheme(inputs: &tl::TlInputs, config: &CodegenScheme) -> Scheme {
    parser::parse_inputs(inputs, config)
}
