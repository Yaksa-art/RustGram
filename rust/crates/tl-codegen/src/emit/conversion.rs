//! `-conversion-{from,to}.h/.cpp` emitters: port of the `writeConversion`
//! blocks — funcs `tl_to_generator` (lines 679-710), types `tl_from`/`tl_to`
//! (833-911), per-ctor `tl_from`/`tl_to` (1094-1156) + banners (1559-1605).
//! Gated on `conversion.is_some()` (`writeConversion`). M5.
//!
//! Dead code for the tdesktop config (`conversion` absent), but ported
//! faithfully so enabling it surfaces the same output — including the
//! `Conversion with flags :(` bail — as Python.

use crate::config::CodegenScheme;
use crate::ir::{Constructor, Scheme};
use crate::parse::normalized_name;

fn full_type(config: &CodegenScheme, name: &str) -> String {
    format!("{}{}", config.prefixes.type_, normalized_name(name))
}

fn conversion_name(config: &CodegenScheme, name: &str) -> String {
    let ns = config
        .conversion
        .as_ref()
        .map(|c| c.namespace.as_str())
        .unwrap_or("");
    format!("::{ns}::{name}")
}

fn capitalize_first(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(f) => format!("{}{}", f.to_uppercase(), ch.as_str()),
        None => String::new(),
    }
}

fn is_cond(c: &Constructor, pname: &str) -> bool {
    c.conditions.iter().any(|(n, _)| n == pname)
}

/// `Conversion with flags :(` — Python raises `ValueError`; we panic with
/// the identical message (conversion runs at build time, same crash).
fn bail_flags() -> ! {
    panic!("Conversion with flags :(")
}

/// One `tl_from_*` argument in `tl_from` bodies (`specific->k_` accessor).
fn from_arg(c: &Constructor, pname: &str, bare: &str, config: &CodegenScheme) -> String {
    if is_cond(c, pname) {
        bail_flags();
    }
    if bare == "string" {
        return format!("tl_from_string(specific->{pname}_)");
    }
    if config.builtin.contains(bare) || bare == "bool" {
        return format!("tl_from_simple(specific->{pname}_)");
    }
    if bare.contains('<') {
        let pfb = full_type(config, bare);
        if c.nullable_vectors.contains(pname) {
            return format!("tl_from_vector_optional<{pfb}>(specific->{pname}_)");
        }
        return format!("tl_from_vector<{pfb}>(specific->{pname}_)");
    }
    let pfb = full_type(config, bare);
    if c.nullable_params.contains(pname) {
        format!("specific->{pname}_.get() ? std::make_optional(tl_from<{pfb}>(specific->{pname}_.get())) : std::nullopt")
    } else {
        format!("tl_from<{pfb}>(specific->{pname}_.get())")
    }
}

/// One `tl_to_*` argument in `tl_to` bodies (`value.vk()` accessor).
fn to_arg(
    c: &Constructor,
    pname: &str,
    bare: &str,
    config: &CodegenScheme,
    scheme: &Scheme,
) -> Option<String> {
    if is_cond(c, pname) {
        bail_flags();
    }
    if c.bots_only_params.contains(pname) {
        return Some("{}".to_string());
    }
    if config.builtin.contains(bare) || bare == "bool" {
        return Some(format!("tl_to_simple(value.v{pname}())"));
    }
    if bare.contains('<') {
        if c.nullable_vectors.contains(pname) {
            return Some(format!("tl_to_vector_optional(value.v{pname}())"));
        }
        return Some(format!("tl_to_vector(value.v{pname}())"));
    }
    let conv = if c.nullable_params.contains(pname) {
        format!("value.v{pname}() ? tl_to(*value.v{pname}()) : nullptr")
    } else {
        format!("tl_to(value.v{pname}())")
    };
    let multi = scheme
        .types
        .get(bare)
        .map(|d| d.ctors.len() > 1)
        .unwrap_or(false);
    let target = if multi {
        conversion_name(config, &capitalize_first(bare))
    } else {
        conversion_name(config, bare)
    };
    Some(format!("::td::td_api::object_ptr<{target}>({conv})"))
}

fn params_empty(c: &Constructor) -> bool {
    c.params.is_empty()
}

/// Accumulated conversion text for all four outputs.
#[derive(Default)]
struct Conv {
    header_from: String,
    source_from: String,
    header_to: String,
    source_to: String,
}

fn gen(scheme: &Scheme, config: &CodegenScheme) -> Conv {
    let mut cv = Conv::default();
    let conv = match config.conversion.as_ref() {
        Some(c) => c,
        None => return cv,
    };

    // ---- types first (file order: types before funcs) ----
    for (restype_key, def) in &scheme.types {
        if config.builtin.contains(restype_key) || conv.builtin_additional.contains(restype_key) {
            continue;
        }
        let full = full_type(config, restype_key);
        let nullable = def.nullable;

        if def.ctors.len() > 1 {
            // Union tl_from (833-884).
            cv.header_from.push_str(&format!(
                "template <>\n{full} tl_from<{full}>(ExternalResponse response);\n"
            ));
            cv.source_from.push_str(&format!(
                "\ntemplate <>\n{full} tl_from<{full}>(ExternalResponse response) {{\n"
            ));
            if nullable {
                cv.source_from
                    .push_str("\tif (!response) {\n\t\treturn nullptr;\n\t}\n\n");
            } else {
                cv.source_from
                    .push_str("\tExpects(response != nullptr);\n\n");
            }
            cv.source_from.push_str("\tswitch (response->get_id()) {\n");
            for c in &def.ctors {
                let name = &c.name;
                let cname = conversion_name(config, name);
                cv.source_from.push_str(&format!("\tcase {cname}::ID: "));
                if params_empty(c) {
                    cv.source_from.push_str(&format!(
                        "return {}{}();\n",
                        config.prefixes.construct, name
                    ));
                } else {
                    cv.source_from.push_str(&format!(
                        "{{\n\t\tconst auto specific = static_cast<const {cname}*>(response);\n"
                    ));
                    cv.source_from.push_str(&format!(
                        "\t\treturn {}{}(",
                        config.prefixes.construct, name
                    ));
                    let mut args = Vec::new();
                    for p in &c.params {
                        if c.bots_only_params.contains(&p.name) {
                            continue;
                        }
                        args.push(from_arg(c, &p.name, &p.tl_type, config));
                    }
                    cv.source_from.push_str(&args.join(", "));
                    cv.source_from.push_str(");\n\t} break;\n");
                }
            }
            cv.source_from.push_str(&format!(
                "\tdefault: Unexpected(\"Type in {full} tl_from.\");\n\t}}\n}}\n"
            ));
        }

        // Union/single tl_to decl + source (888-911).
        let conversion_type = if def.ctors.len() > 1 {
            restype_key.clone()
        } else {
            def.ctors[0].name.clone()
        };
        let conv_type_name = conversion_name(config, &conversion_type);
        cv.header_to
            .push_str(&format!("{conv_type_name} *tl_to(const {full} &value);\n"));
        cv.source_to.push_str(&format!(
            "\n{conv_type_name} *tl_to(const {full} &value) {{\n"
        ));
        if def.with_type {
            cv.source_to.push_str("\tswitch (value.type()) {\n");
            if nullable {
                let tid = config.types.type_id.clone();
                cv.source_to
                    .push_str(&format!("\tcase {tid}(0): return nullptr;\n"));
            }
            for c in &def.ctors {
                let name = &c.name;
                let idp = config.id_prefix();
                if c.params.len() == c.trivial_conditions.len() {
                    cv.source_to.push_str(&format!(
                        "\tcase {idp}{name}: return new {}();\n",
                        conversion_name(config, name)
                    ));
                } else {
                    cv.source_to.push_str(&format!(
                        "\tcase {idp}{name}: return tl_to(value.c_{name}());\n"
                    ));
                }
            }
            cv.source_to.push_str(&format!(
                "\tdefault: Unexpected(\"Type in tl_to({full}).\");\n\t}}\n"
            ));
        } else {
            if nullable {
                cv.source_to
                    .push_str("\tif (!value) {\n\t\treturn nullptr;\n\t}\n");
            }
            let first = &def.ctors[0].name;
            cv.source_to
                .push_str(&format!("\treturn tl_to(value.c_{first}());\n"));
        }
        cv.source_to.push_str("}\n");

        // Per-ctor blocks (1094-1156).
        for c in &def.ctors {
            let name = &c.name;
            let data_full = format!("{}{}", config.prefixes.data, normalized_name(name));
            if def.ctors.len() == 1 {
                cv.header_from.push_str(&format!(
                    "template <>\n{full} tl_from<{full}>(ExternalResponse response);\n"
                ));
                cv.source_from.push_str(&format!(
                    "\ntemplate <>\n{full} tl_from<{full}>(ExternalResponse response) {{\n"
                ));
                if nullable {
                    cv.source_from
                        .push_str("\tif (!response) {\n\t\treturn nullptr;\n\t}\n\n");
                } else {
                    cv.source_from.push_str("\tExpects(response != nullptr);\n");
                    cv.source_from.push_str(&format!(
                        "\tExpects(response->get_id() == {}::ID);\n\n",
                        conversion_name(config, name)
                    ));
                }
                if !c.params.is_empty() {
                    cv.source_from.push_str(&format!(
                        "\tconst auto specific = static_cast<const {}*>(response);\n",
                        conversion_name(config, name)
                    ));
                }
                cv.source_from.push_str(&format!(
                    "\treturn {}{}(",
                    config.prefixes.construct,
                    normalized_name(name)
                ));
                let mut args = Vec::new();
                for p in &c.params {
                    if c.bots_only_params.contains(&p.name) {
                        continue;
                    }
                    args.push(from_arg(c, &p.name, &p.tl_type, config));
                }
                cv.source_from.push_str(&args.join(", "));
                cv.source_from.push_str(");\n}\n");
            }
            // Per-data-class tl_to (1130-1156).
            let cname = conversion_name(config, name);
            cv.header_to
                .push_str(&format!("{cname} *tl_to(const {data_full} &value);\n"));
            cv.source_to
                .push_str(&format!("\n{cname} *tl_to(const {data_full} &value) {{\n"));
            cv.source_to.push_str(&format!("\treturn new {cname}("));
            let mut args = Vec::new();
            for p in &c.params {
                if let Some(a) = to_arg(c, &p.name, &p.tl_type, config, scheme) {
                    args.push(a);
                }
            }
            cv.source_to.push_str(&args.join(", "));
            cv.source_to.push_str(");\n}\n");
        }
    }

    // ---- funcs tl_to_generator (679-710) ----
    for def in scheme.funcs.values() {
        for c in &def.ctors {
            let name = &c.name;
            let full = full_type(config, name);
            let cname = conversion_name(config, name);
            cv.source_to.push_str(&format!(
                "\ntemplate <>\nExternalGenerator tl_to_generator({full} &&request) {{\n\treturn [value = std::move(request)]() -> ExternalRequest {{\n\t\treturn new {cname}("
            ));
            let mut args = Vec::new();
            for p in &c.params {
                if let Some(a) = to_arg(c, &p.name, &p.tl_type, config, scheme) {
                    args.push(a);
                }
            }
            cv.source_to.push_str(&args.join(", "));
            cv.source_to.push_str(");\n\t};\n}\n");
        }
    }

    cv
}

fn banner(title_inputs: &Scheme, extra_head: &str, body: &str, extra_tail: &str) -> String {
    let input_names = title_inputs
        .input_names
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "// WARNING! All changes made in this file will be lost!\n// Created from {input_names} by 'generate.py'\n//\n{extra_head}{body}{extra_tail}"
    )
}

pub fn emit_from_header(scheme: &Scheme, config: &CodegenScheme) -> String {
    emit_from_header_with_basenames(scheme, config, "scheme.h")
}

pub fn emit_from_header_with_basenames(
    scheme: &Scheme,
    config: &CodegenScheme,
    scheme_header_basename: &str,
) -> String {
    let cv = gen(scheme, config);
    let conv = match config.conversion.as_ref() {
        Some(c) => c,
        None => return String::new(),
    };
    let global = config.namespaces.global.as_str();
    let mut head = "#pragma once\n\n".to_string();
    head.push_str(&format!("#include \"{}\"\n", conv.include));
    head.push_str(&format!("#include \"{scheme_header_basename}\"\n\n"));
    if !global.is_empty() {
        head.push_str(&format!("namespace {global} {{\n\n"));
    }
    let mut tail = String::new();
    if !global.is_empty() {
        tail.push_str(&format!("}} // namespace {global}\n"));
    }
    if !conv.builtin_include_from.is_empty() {
        tail.push_str(&format!("\n#include \"{}\"\n", conv.builtin_include_from));
    }
    banner(scheme, &head, &cv.header_from, &tail)
}

pub fn emit_to_header(scheme: &Scheme, config: &CodegenScheme) -> String {
    emit_to_header_with_basenames(scheme, config, "scheme.h")
}

pub fn emit_to_header_with_basenames(
    scheme: &Scheme,
    config: &CodegenScheme,
    scheme_header_basename: &str,
) -> String {
    let cv = gen(scheme, config);
    let conv = match config.conversion.as_ref() {
        Some(c) => c,
        None => return String::new(),
    };
    let global = config.namespaces.global.as_str();
    let mut head = "#pragma once\n\n".to_string();
    head.push_str(&format!("#include \"{}\"\n", conv.include));
    head.push_str(&format!("#include \"{scheme_header_basename}\"\n\n"));
    if !global.is_empty() {
        head.push_str(&format!("namespace {global} {{\n\n"));
    }
    let mut tail = String::new();
    if !global.is_empty() {
        tail.push_str(&format!("}} // namespace {global}\n"));
    }
    if !conv.builtin_include_to.is_empty() {
        tail.push_str(&format!("\n#include \"{}\"\n", conv.builtin_include_to));
    }
    banner(scheme, &head, &cv.header_to, &tail)
}

pub fn emit_from_source(scheme: &Scheme, config: &CodegenScheme) -> String {
    emit_from_source_with_basenames(scheme, config, "scheme-conversion-from.h")
}

pub fn emit_from_source_with_basenames(
    scheme: &Scheme,
    config: &CodegenScheme,
    from_header_basename: &str,
) -> String {
    if config.conversion.is_none() {
        return String::new();
    }
    let cv = gen(scheme, config);
    let global = config.namespaces.global.as_str();
    let mut head = format!("#include \"{from_header_basename}\"\n\n");
    if !global.is_empty() {
        head.push_str(&format!("namespace {global} {{\n"));
    }
    let mut tail = String::new();
    if !global.is_empty() {
        tail.push_str(&format!("}} // namespace {global}\n"));
    }
    banner(scheme, &head, &cv.source_from, &tail)
}

pub fn emit_to_source(scheme: &Scheme, config: &CodegenScheme) -> String {
    emit_to_source_with_basenames(scheme, config, "scheme-conversion-to.h")
}

pub fn emit_to_source_with_basenames(
    scheme: &Scheme,
    config: &CodegenScheme,
    to_header_basename: &str,
) -> String {
    if config.conversion.is_none() {
        return String::new();
    }
    let cv = gen(scheme, config);
    let global = config.namespaces.global.as_str();
    let mut head = format!("#include \"{to_header_basename}\"\n\n");
    if !global.is_empty() {
        head.push_str(&format!("namespace {global} {{\n"));
    }
    let mut tail = String::new();
    if !global.is_empty() {
        tail.push_str(&format!("}} // namespace {global}\n"));
    }
    banner(scheme, &head, &cv.source_to, &tail)
}
