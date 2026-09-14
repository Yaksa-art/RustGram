//! `-dump_to_text.h/.cpp` emitter: port of `addTextSerialize` (lines
//! 26-151) + `addTextSerializeInit` (154-161). Gated on
//! `dump_to_text.is_some()` (`writeSerialization`). M4.

use crate::config::CodegenScheme;
use crate::ir::Scheme;

#[allow(dead_code)]
pub fn emit_header(_scheme: &Scheme, _config: &CodegenScheme) -> String {
    String::new()
}

#[allow(dead_code)]
pub fn emit_source(_scheme: &Scheme, _config: &CodegenScheme) -> String {
    String::new()
}
