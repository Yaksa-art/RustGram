//! `-dump_to_text.h/.cpp` emitter: port of `addTextSerialize` (lines
//! 26-151) + `addTextSerializeInit` (154-161) + manual `rpc_result` /
//! `msg_container` / `core_message` blocks and the `DumpToTextType` driver
//! (1366-1490) + banners (1607-1633). Gated on `dump_to_text.is_some()`
//! (`writeSerialization`). M4.

use crate::config::CodegenScheme;
use crate::ir::{Constructor, Scheme};
use crate::parse::normalized_name;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

fn vector_mtp_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[Vv]ector<MTP([A-Za-z0-9\._]+)>").unwrap())
}

fn upper_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Z]").unwrap())
}

fn full_type(config: &CodegenScheme, name: &str) -> String {
    format!("{}{}", config.prefixes.type_, normalized_name(name))
}

fn full_data(config: &CodegenScheme, name: &str) -> String {
    format!("{}{}", config.prefixes.data, normalized_name(name))
}

/// `boxed` map: Python sets `boxed[resType] = restype` and `boxed[Name] =
/// name` for every parsed line (types and funcs).
fn build_boxed(scheme: &Scheme) -> HashMap<String, String> {
    let mut boxed = HashMap::new();
    for table in [&scheme.types, &scheme.funcs] {
        for (restype_key, def) in table {
            boxed.insert(def.boxed_name.clone(), restype_key.clone());
            for c in &def.ctors {
                boxed.insert(c.alias_name.clone(), c.name.clone());
            }
        }
    }
    boxed
}

/// `addTextSerialize`: one `Serialize_<name>` function per constructor.
#[allow(clippy::too_many_arguments)]
fn add_text_serialize(
    type_list: &[String],
    type_data: &indexmap::IndexMap<String, crate::ir::TypeDef>,
    types_dict: &indexmap::IndexMap<String, crate::ir::TypeDef>,
    idp: &str,
    prime: &str,
    boxed: &HashMap<String, String>,
    prefix: &str,
) -> String {
    let mut result = String::new();
    for restype in type_list {
        let def = match type_data.get(restype) {
            Some(d) => d,
            None => continue,
        };
        for c in &def.ctors {
            serialize_ctor(&mut result, c, types_dict, idp, prime, boxed, prefix);
        }
    }
    result
}

fn serialize_ctor(
    result: &mut String,
    c: &Constructor,
    types_dict: &indexmap::IndexMap<String, crate::ir::TypeDef>,
    idp: &str,
    prime: &str,
    boxed: &HashMap<String, String>,
    prefix: &str,
) {
    let name = &c.name;
    // `prmsList` order; skip trivial/bots params exactly like the stored
    // check `len(prms) > len(trivial) + len(bots)` implies for iteration —
    // note Python iterates ALL of prmsList here (no trivial/bots skip).
    let prms_list: Vec<&crate::ir::Param> = c.params.iter().collect();
    let has_flags = c.flags_name.as_str();
    let has_flags64 = c.flags64_name.as_str();
    let flag_raw_type = if !has_flags64.is_empty() {
        "uint64"
    } else {
        "uint32"
    };
    let template_argument = if c.is_template {
        "<SerializedRequest>"
    } else {
        ""
    };

    result.push_str(&format!(
        "bool Serialize_{name}(DumpToTextBuffer &to, int32 stage, int32 lev, Types &types, Types &vtypes, Stages &stages, Flags &flags, const {prime} *start, const {prime} *end, uint64 iflag) {{\n"
    ));
    if !c.conditions.is_empty() {
        result.push_str(&format!(
            "\tauto flag = {prefix}{name}{template_argument}::Flags::from_raw({flag_raw_type}(iflag));\n\n"
        ));
    }
    if !prms_list.is_empty() {
        let mut not_first = "stage".to_string();
        if !c.conditions.is_empty() {
            not_first = "!first".to_string();
            result.push_str("\tconst auto first = !stage;\n");
            result.push_str("\tconst auto absent = [&](int32 index) {\n");
            result.push_str("\t\tswitch (index) {\n");
            let mut stage = 0;
            for p in &prms_list {
                let k = &p.name;
                if c.conditions.iter().any(|(n, _)| n == k) {
                    result.push_str(&format!(
                        "\t\tcase {stage}: return !(flag & {prefix}{name}{template_argument}::Flag::f_{k});\n"
                    ));
                }
                stage += 1;
            }
            result.push_str("\t\t}\n");
            result.push_str("\t\treturn false;\n");
            result.push_str("\t};\n");
            result.push_str("\twhile (absent(stage)) {\n");
            result.push_str("\t\t++stage;\n");
            result.push_str("\t\t++stages.back();\n");
            result.push_str("\t}\n\n");
        }
        result.push_str(&format!("\tif ({not_first}) {{\n"));
        result.push_str("\t\tto.add(\",\\n\").addSpaces(lev);\n");
        result.push_str("\t} else {\n");
        result.push_str(&format!("\t\tto.add(\"{{ {name}\");\n"));
        result.push_str("\t\tto.add(\"\\n\").addSpaces(lev);\n");
        result.push_str("\t}\n");
        result.push_str("\tswitch (stage) {\n");
        let mut stage = 0;
        for p in &prms_list {
            let k = &p.name;
            let v = &p.tl_type;
            result.push_str(&format!(
                "\tcase {stage}: to.add(\"  {k}: \"); ++stages.back(); "
            ));
            if *k == *has_flags {
                if !has_flags64.is_empty() {
                    result.push_str("if (start + 1 >= end) return false; else flags.back() = int64(*start) + (int64(*(start + 1)) << 32); ");
                } else {
                    result.push_str("if (start >= end) return false; else flags.back() = *start; ");
                }
            }
            if c.trivial_conditions.contains(k) {
                let bit: i64 = c
                    .conditions
                    .iter()
                    .find(|(n, _)| n == k)
                    .map(|(_, b)| *b as i64)
                    .unwrap_or(0);
                let field = if bit >= 32 { has_flags64 } else { has_flags };
                let logged = if bit >= 32 { bit - 32 } else { bit };
                result.push_str(&format!(
                    "to.add(\"YES [ BY BIT {logged} IN FIELD {field} ]\"); "
                ));
            } else {
                result.push_str("types.push_back(");
                let vtypeget = vector_mtp_re().captures(v).map(|cap| cap[1].to_string());
                let mut restype_opt: Option<String> = None;
                if let Some(ref inner) = vtypeget {
                    if !upper_re().is_match(v) {
                        result.push_str(&format!("{idp}vector"));
                    } else {
                        result.push('0');
                    }
                    let mut rt = inner.clone();
                    if boxed.get(&rt).is_some() {
                        rt.clear();
                    } else if upper_re().is_match(&rt) {
                        rt.clear();
                    }
                    restype_opt = Some(rt);
                } else {
                    let mut rt = v.clone();
                    if boxed.get(&rt).is_some() {
                        rt.clear();
                    } else if upper_re().is_match(&rt) {
                        rt.clear();
                    }
                    restype_opt = Some(rt);
                }
                let restype = restype_opt.unwrap_or_default();
                if !restype.is_empty() {
                    match types_dict.get(&restype) {
                        Some(conses) => {
                            if conses.ctors.len() > 1 {
                                // Python prints and `continue`s (skipping the
                                // push/break/stage++ for this param).
                                eprintln!(
                                    "Complex bare type found: \"{restype}\" trying to serialize \"{k}\" of type \"{v}\""
                                );
                                continue;
                            }
                            if vtypeget.is_some() {
                                result.push_str("); vtypes.push_back(");
                            }
                            let first_ctor = &conses.ctors[0].name;
                            result.push_str(&format!("{idp}{first_ctor}"));
                            if vtypeget.is_none() {
                                result.push_str("); vtypes.push_back(0");
                            }
                        }
                        None => {
                            if vtypeget.is_some() {
                                result.push_str("); vtypes.push_back(");
                            }
                            if restype.starts_with("flags<") {
                                result.push_str(&format!("{idp}flags"));
                                if *k == *has_flags && !has_flags64.is_empty() {
                                    result.push_str("64");
                                }
                            } else {
                                result.push_str(&format!("{idp}{restype}+0"));
                            }
                            if vtypeget.is_none() {
                                result.push_str("); vtypes.push_back(0");
                            }
                        }
                    }
                } else {
                    if vtypeget.is_none() {
                        result.push('0');
                    }
                    result.push_str("); vtypes.push_back(0");
                }
                result.push_str("); stages.push_back(0); flags.push_back(0); ");
            }
            result.push_str("break;\n");
            stage += 1;
        }
        result.push_str("\tdefault: to.add(\"}\"); types.pop_back(); vtypes.pop_back(); stages.pop_back(); flags.pop_back(); break;\n");
        result.push_str("\t}\n");
    } else {
        result.push_str(&format!(
            "\tto.add(\"{{ {name} }}\"); types.pop_back(); vtypes.pop_back(); stages.pop_back(); flags.pop_back();\n"
        ));
    }
    result.push_str("\treturn true;\n");
    result.push_str("}\n\n");
}

/// `addTextSerializeInit`: `{ mtpc<name>, Serialize_<name> },` rows.
fn add_text_serialize_init(
    type_list: &[String],
    type_data: &indexmap::IndexMap<String, crate::ir::TypeDef>,
    idp: &str,
) -> String {
    let mut result = String::new();
    for restype in type_list {
        if let Some(def) = type_data.get(restype) {
            for c in &def.ctors {
                result.push_str(&format!(
                    "\t\t{{ {idp}{}, Serialize_{} }},\n",
                    c.name, c.name
                ));
            }
        }
    }
    result
}

fn manual_methods(idp: &str) -> String {
    format!(
        "bool Serialize_rpc_result(DumpToTextBuffer &to, int32 stage, int32 lev, Types &types, Types &vtypes, Stages &stages, Flags &flags, const {{PRIME}} *start, const {{PRIME}} *end, uint64 iflag) {{\n\
\tif (stage) {{\n\
\t\tto.add(\",\\n\").addSpaces(lev);\n\
\t}} else {{\n\
\t\tto.add(\"{{ rpc_result\");\n\
\t\tto.add(\"\\n\").addSpaces(lev);\n\
\t}}\n\
\tswitch (stage) {{\n\
\tcase 0: to.add(\"  req_msg_id: \"); ++stages.back(); types.push_back({idp}long); vtypes.push_back(0); stages.push_back(0); flags.push_back(0); break;\n\
\tcase 1: to.add(\"  result: \"); ++stages.back(); types.push_back(0); vtypes.push_back(0); stages.push_back(0); flags.push_back(0); break;\n\
\tdefault: to.add(\" }}\"); types.pop_back(); vtypes.pop_back(); stages.pop_back(); flags.pop_back(); break;\n\
\t}}\n\
\treturn true;\n\
}}\n\
\n\
bool Serialize_msg_container(DumpToTextBuffer &to, int32 stage, int32 lev, Types &types, Types &vtypes, Stages &stages, Flags &flags, const {{PRIME}} *start, const {{PRIME}} *end, uint64 iflag) {{\n\
\tif (stage) {{\n\
\t\tto.add(\",\\n\").addSpaces(lev);\n\
\t}} else {{\n\
\t\tto.add(\"{{ msg_container\");\n\
\t\tto.add(\"\\n\").addSpaces(lev);\n\
\t}}\n\
\tswitch (stage) {{\n\
\tcase 0: to.add(\"  messages: \"); ++stages.back(); types.push_back({idp}vector); vtypes.push_back({idp}core_message); stages.push_back(0); flags.push_back(0); break;\n\
\tdefault: to.add(\" }}\"); types.pop_back(); vtypes.pop_back(); stages.pop_back(); flags.pop_back(); break;\n\
\t}}\n\
\treturn true;\n\
}}\n\
\n\
bool Serialize_core_message(DumpToTextBuffer &to, int32 stage, int32 lev, Types &types, Types &vtypes, Stages &stages, Flags &flags, const {{PRIME}} *start, const {{PRIME}} *end, uint64 iflag) {{\n\
\tif (stage) {{\n\
\t\tto.add(\",\\n\").addSpaces(lev);\n\
\t}} else {{\n\
\t\tto.add(\"{{ core_message\");\n\
\t\tto.add(\"\\n\").addSpaces(lev);\n\
\t}}\n\
\tswitch (stage) {{\n\
\tcase 0: to.add(\"  msg_id: \"); ++stages.back(); types.push_back({idp}long); vtypes.push_back(0); stages.push_back(0); flags.push_back(0); break;\n\
\tcase 1: to.add(\"  seq_no: \"); ++stages.back(); types.push_back({idp}int); vtypes.push_back(0); stages.push_back(0); flags.push_back(0); break;\n\
\tcase 2: to.add(\"  bytes: \"); ++stages.back(); types.push_back({idp}int); vtypes.push_back(0); stages.push_back(0); flags.push_back(0); break;\n\
\tcase 3: to.add(\"  body: \"); ++stages.back(); types.push_back(0); vtypes.push_back(0); stages.push_back(0); flags.push_back(0); break;\n\
\tdefault: to.add(\" }}\"); types.pop_back(); vtypes.pop_back(); stages.pop_back(); flags.pop_back(); break;\n\
\t}}\n\
\treturn true;\n\
}}\n\
\n"
    )
}

fn manual_init(idp: &str) -> String {
    format!(
        "\t    {{ {idp}rpc_result, Serialize_rpc_result }},\n\
\t    {{ {idp}msg_container, Serialize_msg_container }},\n\
\t    {{ {idp}core_message, Serialize_core_message }},\n"
    )
}

fn text_serialize_source(methods: &str, init: &str, prime: &str, type_id: &str) -> String {
    format!(
        "namespace {{\n\
\n\
using Types = QVector<{type_id}>;\n\
using Stages = QVector<int32>;\n\
using Flags = QVector<int64>;\n\
\n\
{methods}\n\
\n\
using TextSerializer = bool (*)(DumpToTextBuffer &to, int32 stage, int32 lev, Types &types, Types &vtypes, Stages &stages, Flags &flags, const {prime} *start, const {prime} *end, uint64 iflag);\n\
\n\
base::flat_map<{type_id}, TextSerializer> CreateTextSerializers() {{\n\
\treturn {{\n\
{init}\n\
\t}};\n\
}}\n\
\n\
}} // namespace\n\
\n\
bool DumpToTextType(DumpToTextBuffer &to, const {prime} *&from, const {prime} *end, {prime} cons, uint32 level, {prime} vcons) {{\n\
\tstatic auto kSerializers = CreateTextSerializers();\n\
\n\
\tTypes types, vtypes;\n\
\tStages stages;\n\
\tFlags flags;\n\
\ttypes.reserve(20); vtypes.reserve(20); stages.reserve(20); flags.reserve(20);\n\
\ttypes.push_back({type_id}(cons)); vtypes.push_back({type_id}(vcons)); stages.push_back(0); flags.push_back(0);\n\
\n\
\t{type_id} type = cons, vtype = vcons;\n\
\tint32 stage = 0;\n\
\tint64 flag = 0;\n\
\n\
\twhile (!types.isEmpty()) {{\n\
\t\ttype = types.back();\n\
\t\tvtype = vtypes.back();\n\
\t\tstage = stages.back();\n\
\t\tflag = flags.back();\n\
\t\tif (!type) {{\n\
\t\t\tif (from >= end) {{\n\
\t\t\t\tto.error(\"insufficient data\");\n\
\t\t\t\treturn false;\n\
\t\t\t}}\n\
\t\t\telse if (stage) {{\n\
\t\t\t\tto.error(\"unknown type on stage > 0\");\n\
\t\t\t\treturn false;\n\
\t\t\t}}\n\
\t\t\ttypes.back() = type = *from;\n\
\t\t\t++from;\n\
\t\t}}\n\
\n\
\t\tint32 lev = level + types.size() - 1;\n\
\t\tauto it = kSerializers.find(type);\n\
\t\tif (it != kSerializers.end()) {{\n\
\t\t\tif (!(*it->second)(to, stage, lev, types, vtypes, stages, flags, from, end, flag)) {{\n\
\t\t\t\tto.error();\n\
\t\t\t\treturn false;\n\
\t\t\t}}\n\
\t\t}} else if (DumpToTextCore(to, from, end, type, lev, vtype)) {{\n\
\t\t\ttypes.pop_back(); vtypes.pop_back(); stages.pop_back(); flags.pop_back();\n\
\t\t}} else {{\n\
\t\t\tto.error();\n\
\n\
\t\t\tconst auto bad = QByteArray::number(type, 16);\n\
\t\t\tto.add(\"(ERROR_SCHEME_BAD_CONS:0x\").add(bad.data()).add(\")\");\n\
\n\
\t\t\treturn false;\n\
\t\t}}\n\
\t}}\n\
\treturn true;\n\
}}"
    )
}

pub fn emit_header(scheme: &Scheme, config: &CodegenScheme) -> String {
    let _ = scheme;
    let prime = config.types.prime.as_str();
    let mut h = String::new();
    h.push_str("// WARNING! All changes made in this file will be lost!\n");
    let input_names = scheme
        .input_names
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    h.push_str(&format!(
        "// Created from {input_names} by 'generate.py'\n//\n#pragma once\n\n"
    ));
    if !config.builtin_include.is_empty() {
        h.push_str(&format!("#include \"{}\"\n\n", config.builtin_include));
    }
    h.push_str("namespace MTP::details {\n\nstruct DumpToTextBuffer;\n\n");
    h.push_str(&format!(
        "[[nodiscard]] bool DumpToTextType(DumpToTextBuffer &to, const {prime} *&from, const {prime} *end, {prime} cons = 0, uint32 level = 0, {prime} vcons = 0);\n\n}} // namespace MTP::details\n"
    ));
    h
}

pub fn emit_source(scheme: &Scheme, config: &CodegenScheme) -> String {
    emit_source_with_basenames(scheme, config, "scheme-dump_to_text.h", "scheme.h")
}

pub fn emit_source_with_basenames(
    scheme: &Scheme,
    config: &CodegenScheme,
    dump_header_basename: &str,
    scheme_header_basename: &str,
) -> String {
    let idp = config.id_prefix();
    let prime = config.types.prime.as_str();
    let type_id = config.types.type_id.as_str();
    let data_prefix = config.prefixes.data.as_str();
    let type_prefix = config.prefixes.type_.as_str();
    let boxed = build_boxed(scheme);

    let types_list: Vec<String> = scheme.types.keys().cloned().collect();
    let funcs_list: Vec<String> = scheme.funcs.keys().cloned().collect();

    let mut methods = add_text_serialize(
        &types_list,
        &scheme.types,
        &scheme.types,
        &idp,
        prime,
        &boxed,
        data_prefix,
    );
    let mut init = add_text_serialize_init(&types_list, &scheme.types, &idp);
    methods.push_str(&add_text_serialize(
        &funcs_list,
        &scheme.funcs,
        &scheme.types,
        &idp,
        prime,
        &boxed,
        type_prefix,
    ));
    init.push_str(&add_text_serialize_init(&funcs_list, &scheme.funcs, &idp));

    methods.push_str(&manual_methods(&idp).replace("{PRIME}", prime));
    init.push_str(&manual_init(&idp));

    let source_body = text_serialize_source(&methods, &init, prime, type_id);

    let serialization_include = config
        .dump_to_text
        .as_ref()
        .map(|d| d.include.as_str())
        .unwrap_or("");

    let input_names = scheme
        .input_names
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut s = String::new();
    s.push_str("// WARNING! All changes made in this file will be lost!\n");
    s.push_str(&format!(
        "// Created from {input_names} by 'generate.py'\n//\n#include \"{dump_header_basename}\"\n#include \"{scheme_header_basename}\"\n#include \"{serialization_include}\"\n#include \"base/flat_map.h\"\n\nnamespace MTP::details {{\n{source_body}\n}} // namespace MTP::details\n"
    ));
    // Silence dead-code warnings for helpers shared with future ports.
    let _ = (full_type(config, "x"), full_data(config, "x"));
    s
}
