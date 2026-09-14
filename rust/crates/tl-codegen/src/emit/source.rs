//! `.cpp` emitter: banner assembly (Python lines 1539-1557) over the shared
//! [`gen::generate`] pass. The `#include` uses the output header basename
//! like Python's `outputHeaderBasename`; the driver passes it via
//! [`emit_with_basename`] (plain [`emit`] defaults to `scheme.h`).

use crate::config::CodegenScheme;
use crate::emit::gen;
use crate::ir::Scheme;

pub fn emit(scheme: &Scheme, config: &CodegenScheme) -> String {
    emit_with_basename(scheme, config, "scheme.h")
}

pub fn emit_with_basename(
    scheme: &Scheme,
    config: &CodegenScheme,
    header_basename: &str,
) -> String {
    let g = gen::generate(scheme, config);
    let global = config.namespaces.global.as_str();
    let creator = config.namespaces.creator.as_str();

    let mut s = String::new();
    s.push_str("// WARNING! All changes made in this file will be lost!\n");
    let input_names = scheme
        .input_names
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    s.push_str(&format!(
        "// Created from {input_names} by 'generate.py'\n//\n#include \"{header_basename}\"\n\n"
    ));
    s.push_str("// Creator proxy class definition\n");
    if !global.is_empty() {
        s.push_str(&format!("namespace {global} {{\n"));
    }
    if !creator.is_empty() {
        s.push_str(&format!("namespace {creator} {{\n"));
    }
    s.push_str("\nclass TypeCreator final {\npublic:\n");
    s.push_str(&g.creator_proxy_text);
    s.push_str("\n};\n\n");
    if !creator.is_empty() {
        s.push_str(&format!("}} // namespace {creator}\n\n"));
    }
    s.push_str("// Methods definition\n");
    s.push_str(&g.methods);
    s.push('\n');
    if !global.is_empty() {
        s.push_str(&format!("}} // namespace {global}\n"));
    }
    s
}
