//! `.h` emitter: banner assembly (Python lines 1496-1537) over the shared
//! [`gen::generate`] pass. Byte-identical by construction — raw concatenation,
//! same order, same newlines.

use crate::config::CodegenScheme;
use crate::emit::gen;
use crate::ir::Scheme;

pub fn emit(scheme: &Scheme, config: &CodegenScheme) -> String {
    let g = gen::generate(scheme, config);
    let global = config.namespaces.global.as_str();
    let creator = config.namespaces.creator.as_str();
    // Python: `inputNames = '\'' + '\', \''.join(names) + '\''`.
    let input_names = scheme
        .input_names
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");

    let mut h = String::new();
    h.push_str("// WARNING! All changes made in this file will be lost!\n");
    h.push_str(&format!(
        "// Created from {input_names} by 'generate.py'\n//\n#pragma once\n\n"
    ));
    if !config.builtin_include.is_empty() {
        h.push_str(&format!("#include \"{}\"\n", config.builtin_include));
    }
    h.push_str("#include \"base/assertion.h\"\n");
    h.push_str("#include \"base/flags.h\"\n");
    h.push_str("#include \"tl/tl_boxed.h\"\n");
    h.push_str("#include \"tl/tl_type_owner.h\"\n");
    h.push('\n');
    if !global.is_empty() {
        h.push_str(&format!("namespace {global} {{\n"));
    }
    if !creator.is_empty() {
        h.push_str(&format!("namespace {creator} {{\n"));
    }
    h.push('\n');
    if scheme.layer != 0 {
        h.push_str(&format!(
            "inline constexpr auto kCurrentLayer = {}({});\n\n",
            config.types.prime, scheme.layer
        ));
    }
    h.push_str("class TypeCreator;\n\n");
    if !creator.is_empty() {
        h.push_str(&format!("}} // namespace {creator}\n\n"));
    }
    h.push_str("// Type id constants\nenum {\n");
    h.push_str(&scheme.enums.join(",\n"));
    h.push_str("\n};\n\n// Type forward declarations\n");
    h.push_str(&g.forwards);
    h.push('\n');
    h.push_str(&g.forw_typedefs);
    h.push_str("// Type classes definitions\n");
    h.push_str(&g.types_text);
    h.push('\n');
    h.push_str("// Type constructors with data\n");
    h.push_str(&g.data_texts);
    h.push('\n');
    h.push_str("// RPC methods\n");
    h.push_str(&g.funcs_text);
    h.push('\n');
    h.push_str("// Template methods definition\n");
    h.push_str(&g.inline_methods);
    h.push('\n');
    h.push_str("// Visitor definition\n");
    h.push_str(&g.visitor_methods);
    h.push('\n');
    h.push_str("// Flag operators definition\n");
    h.push_str(&g.flag_operators);
    h.push('\n');
    h.push_str("// Factory methods declaration\n");
    h.push_str(&g.factories);
    h.push('\n');
    if !global.is_empty() {
        h.push_str(&format!("}} // namespace {global}\n"));
    }
    h
}
