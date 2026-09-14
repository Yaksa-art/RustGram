//! `.cpp` emitter (M3). Full port lands here; M0 leaves a stub.

use crate::config::CodegenScheme;
use crate::ir::Scheme;

#[allow(dead_code)]
pub fn emit(_scheme: &Scheme, _config: &CodegenScheme) -> String {
    String::new()
}
