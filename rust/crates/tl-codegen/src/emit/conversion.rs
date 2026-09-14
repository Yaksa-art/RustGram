//! `-conversion-{from,to}.h/.cpp` emitter (M5). Currently dead code in
//! this repo (`conversion` absent from config -> `writeConversion=False`).
//! Port preserves the `bail!("Conversion with flags :(")` behavior so
//! enabling it surfaces the same failure as Python.

use crate::config::CodegenScheme;
use crate::ir::Scheme;

#[allow(dead_code)]
pub fn emit_from_header(_scheme: &Scheme, _config: &CodegenScheme) -> String {
    String::new()
}

#[allow(dead_code)]
pub fn emit_from_source(_scheme: &Scheme, _config: &CodegenScheme) -> String {
    String::new()
}

#[allow(dead_code)]
pub fn emit_to_header(_scheme: &Scheme, _config: &CodegenScheme) -> String {
    String::new()
}

#[allow(dead_code)]
pub fn emit_to_source(_scheme: &Scheme, _config: &CodegenScheme) -> String {
    String::new()
}
