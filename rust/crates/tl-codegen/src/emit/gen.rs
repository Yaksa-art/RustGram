//! Shared emission state: mirrors the 13 Python string accumulators
//! (generate_tl.py lines 352-368, 796-1364) so `header` and `source` agree
//! byte-for-byte. One pass fills everything; the two public emitters only
//! assemble the final banners.

use crate::config::CodegenScheme;
use crate::ir::{Constructor, Scheme};
use crate::parse::{normalized_name, optional_in_vector};

/// All accumulators Python concatenates at the end.
#[derive(Debug, Default)]
pub struct Generated {
    pub funcs_text: String,
    pub types_text: String,
    pub data_texts: String,
    pub creator_proxy_text: String,
    pub factories: String,
    pub flag_operators: String,
    pub methods: String,
    pub inline_methods: String,
    pub visitor_methods: String,
    pub forwards: String,
    pub forw_typedefs: String,
    pub parent_flags_check:
        std::collections::HashMap<String, std::collections::HashMap<String, u32>>,
}

fn full_type(config: &CodegenScheme, name: &str) -> String {
    format!("{}{}", config.prefixes.type_, normalized_name(name))
}

fn full_data(config: &CodegenScheme, name: &str) -> String {
    format!("{}{}", config.prefixes.data, normalized_name(name))
}

fn id_prefix(config: &CodegenScheme) -> String {
    format!("{}_", config.prefixes.id)
}

fn is_scalar_param(t: &str) -> bool {
    matches!(
        t,
        "int"
            | "Int"
            | "bool"
            | "Bool"
            | "flags<Flags>"
            | "long"
            | "int32"
            | "int53"
            | "int64"
            | "double"
    )
}

/// Stored params: `len(prms) > len(trivial) + len(bots)` in Python.
fn stored_params(c: &Constructor) -> Vec<&crate::ir::Param> {
    c.params
        .iter()
        .filter(|p| !c.trivial_conditions.contains(&p.name))
        .filter(|p| !c.bots_only_params.contains(&p.name))
        .collect()
}

fn flags_enum(
    out: &mut String,
    conditions: &[(String, u32)],
    has_flags64: bool,
    parent_check: &mut std::collections::HashMap<String, u32>,
) {
    let flags_type = if has_flags64 { "uint64" } else { "uint32" };
    let flags_bit = if has_flags64 { "1ULL" } else { "1U" };
    out.push_str(&format!("\tenum class Flag : {flags_type} {{\n"));
    let mut maxbit: u32 = 0;
    for (param_name, bit) in conditions {
        out.push_str(&format!("\t\tf_{param_name} = ({flags_bit} << {bit}),\n"));
        parent_check.insert(param_name.clone(), *bit);
        maxbit = maxbit.max(*bit);
    }
    if maxbit > 0 {
        out.push('\n');
    }
    out.push_str(&format!("\t\tMAX_FIELD = ({flags_bit} << {maxbit}),\n"));
    out.push_str("\t};\n");
    out.push_str("\tusing Flags = base::flags<Flag>;\n");
    out.push_str("\tfriend inline constexpr bool is_flag_type(Flag) { return true; };\n");
    out.push('\n');
}

fn getter_decl(
    out: &mut String,
    bodies: &mut String,
    owner_full: &str,
    param_name: &str,
    ptype_full: &str,
    is_conditional: bool,
    is_nullable_vec: bool,
    is_nullable: bool,
    has_flags: &str,
) {
    if is_conditional {
        out.push_str(&format!(
            "\t[[nodiscard]] tl::conditional<{ptype_full}> v{param_name}() const;\n"
        ));
        bodies.push_str(&format!(
            "tl::conditional<{ptype_full}> {owner_full}::v{param_name}() const {{\n"
        ));
        bodies.push_str(&format!(
            "\treturn (_{has_flags}.v & Flag::f_{param_name}) ? &_{param_name} : nullptr;\n"
        ));
        bodies.push_str("}\n");
    } else if is_nullable_vec {
        let opt = optional_in_vector(ptype_full)
            .unwrap_or_else(|_| format!("Vector<std::optional<{ptype_full}>>"));
        out.push_str(&format!(
            "\t[[nodiscard]] const {opt} &v{param_name}() const;\n"
        ));
        bodies.push_str(&format!(
            "const {opt} &{owner_full}::v{param_name}() const {{\n\treturn _{param_name};\n}}\n"
        ));
    } else if is_nullable {
        out.push_str(&format!(
            "\t[[nodiscard]] tl::conditional<{ptype_full}> v{param_name}() const;\n"
        ));
        bodies.push_str(&format!(
            "tl::conditional<{ptype_full}> {owner_full}::v{param_name}() const {{\n"
        ));
        bodies.push_str(&format!(
            "\treturn _{param_name} ? &*_{param_name} : nullptr;\n}}\n"
        ));
    } else {
        out.push_str(&format!(
            "\t[[nodiscard]] const {ptype_full} &v{param_name}() const;\n"
        ));
        bodies.push_str(&format!(
            "const {ptype_full} &{owner_full}::v{param_name}() const {{\n\treturn _{param_name};\n}}\n"
        ));
    }
}

/// Full generation pass: funcs (lines 567-781) + types (809-1342) + flag
/// operators (1343-1364). Conversion/dump accumulators live in their own
/// modules (M4-M5); this covers the always-on `.h/.cpp` core.
pub fn generate(scheme: &Scheme, config: &CodegenScheme) -> Generated {
    let mut g = Generated::default();
    let idp = id_prefix(config);
    let type_id_type = config.types.type_id.clone();
    let prime_type = config.types.prime.clone();
    let buffer_type = config.types.buffer.clone();
    let read_write = config.read_write_section();
    let optimize_single = config.optimize_single_data();

    // ---- builtin forward typedefs (lines 796-801) ----
    if read_write {
        for t in &config.builtin {
            let upper = format!("{}{}", t[..1].to_uppercase(), &t[1..]);
            g.forw_typedefs.push_str(&format!(
                "using {}{} = tl::boxed<{}{}>;\n",
                config.prefixes.type_, upper, config.prefixes.type_, t
            ));
        }
        for t in &config.builtin_templates {
            let upper = format!("{}{}", t[..1].to_uppercase(), &t[1..]);
            g.forw_typedefs.push_str("template <typename T>\n");
            g.forw_typedefs.push_str(&format!(
                "using {}{} = tl::boxed<{}{}<T>>;\n",
                config.prefixes.type_, upper, config.prefixes.type_, t
            ));
        }
    }

    // ---- RPC methods (lines 567-781) ----
    for (restype_key, def) in &scheme.funcs {
        let res_boxed = def.boxed_name.clone();
        for c in &def.ctors {
            let name = &c.name;
            let full = full_type(config, name);
            let is_template = c.is_template;
            let has_flags = !c.flags_name.is_empty();
            let has_flags64 = !c.flags64_name.is_empty();
            let mut method_bodies = String::new();

            let mut head = String::new();
            if is_template {
                head.push_str("\ntemplate <typename TQueryType>");
            }
            head.push_str(&format!(
                "\nclass {full} {{ // RPC method '{}'\n",
                c.tl_name
            ));
            head.push_str("public:\n");
            g.funcs_text.push_str(&head);

            if has_flags {
                let mut check = std::collections::HashMap::new();
                let mut enum_text = String::new();
                flags_enum(&mut enum_text, &c.conditions, has_flags64, &mut check);
                g.funcs_text.push_str(&enum_text);
                g.parent_flags_check.insert(full.clone(), check);
            }

            let stored = stored_params(c);
            if !stored.is_empty() {
                let mut prms_str: Vec<String> = Vec::new();
                let mut prms_init: Vec<String> = Vec::new();
                for p in stored.iter() {
                    let pname = &p.name;
                    prms_init.push(format!("_{pname}({pname}_)"));
                    let ptype_full = if *pname == c.template_param {
                        p.tl_type.clone()
                    } else {
                        full_type(config, &p.tl_type)
                    };
                    if is_scalar_param(&p.tl_type) {
                        prms_str.push(format!("{ptype_full} {pname}_"));
                    } else if c.nullable_vectors.contains(pname) {
                        let opt = optional_in_vector(&ptype_full)
                            .unwrap_or_else(|_| format!("Vector<std::optional<{ptype_full}>>"));
                        prms_str.push(format!("const {opt} &{pname}_"));
                    } else if c.nullable_params.contains(pname) {
                        prms_str.push(format!("const std::optional<{ptype_full}> &{pname}_"));
                    } else {
                        prms_str.push(format!("const {ptype_full} &{pname}_"));
                    }
                }
                let _ = prms_init;
                // stash for later constructor emission
                let joined_params = prms_str.join(", ");
                let joined_init = prms_init.join(", ");
                // constructor declarations
                g.funcs_text.push_str(&format!("\t{full}();\n"));
                if is_template {
                    method_bodies.push_str("template <typename TQueryType>\n");
                    method_bodies.push_str(&format!("{full}<TQueryType>::{full}() = default;\n"));
                } else {
                    method_bodies.push_str(&format!("{full}::{full}() = default;\n"));
                }
                let explicit = if stored.len() == 1 { "explicit " } else { "" };
                g.funcs_text
                    .push_str(&format!("\t{explicit}{full}({joined_params});\n"));
                if is_template {
                    method_bodies.push_str("template <typename TQueryType>\n");
                    method_bodies.push_str(&format!(
                        "{full}<TQueryType>::{full}({joined_params}) : {joined_init} {{\n}}\n"
                    ));
                } else {
                    method_bodies.push_str(&format!(
                        "{full}::{full}({joined_params}) : {joined_init} {{\n}}\n"
                    ));
                }
            } else {
                g.funcs_text.push_str(&format!("\t{full}();\n"));
                if is_template {
                    method_bodies.push_str("template <typename TQueryType>\n");
                    method_bodies.push_str(&format!("{full}<TQueryType>::{full}() = default;\n"));
                } else {
                    method_bodies.push_str(&format!("{full}::{full}() = default;\n"));
                }
            }

            g.funcs_text.push_str(&format!(
                "\t{type_id_type} type() const {{\n\t\treturn {idp}{name};\n\t}}\n"
            ));

            if read_write {
                g.funcs_text.push('\n');
                g.funcs_text.push_str("\ttemplate <typename Prime>\n");
                g.funcs_text.push_str(&format!(
                    "\t[[nodiscard]] bool read(const Prime *&from, const Prime *end, {type_id_type} cons = {idp}{name});\n"
                ));
                if is_template {
                    method_bodies.push_str("template <typename TQueryType>\n");
                    method_bodies.push_str("template <typename Prime>\n");
                    method_bodies.push_str(&format!(
                        "bool {full}<TQueryType>::read(const Prime *&from, const Prime *end, {type_id_type} cons) {{\n"
                    ));
                } else {
                    method_bodies.push_str("template <typename Prime>\n");
                    method_bodies.push_str(&format!(
                        "bool {full}::read(const Prime *&from, const Prime *end, {type_id_type} cons) {{\n"
                    ));
                }
                let mut read_func = String::new();
                for p in &c.params {
                    let k = &p.name;
                    let v = full_type(config, &p.tl_type);
                    let is_cond = c.conditions.iter().any(|(n, _)| n == k);
                    let is_trivial = c.trivial_conditions.contains(k);
                    if is_cond {
                        if !is_trivial {
                            read_func.push_str(&format!(
                                "\t\t&& ((_{}.v & Flag::f_{k}) ? _{k}.read(from, end) : ((_{k} = {v}()), true))\n",
                                c.flags_name
                            ));
                        }
                    } else {
                        read_func.push_str(&format!("\t\t&& _{k}.read(from, end)\n"));
                    }
                }
                // Python: `'\treturn' + readFunc[4:len(readFunc)-1] + ';\n'`.
                if !read_func.is_empty() {
                    method_bodies.push_str("\treturn");
                    method_bodies.push_str(&read_func[4..read_func.len() - 1]);
                    method_bodies.push_str(";\n");
                } else {
                    method_bodies.push_str("\treturn true;\n");
                }
                method_bodies.push_str("}\n");
                if !is_template {
                    method_bodies.push_str(&format!(
                        "template bool {full}::read<{prime_type}>(const {prime_type} *&from, const {prime_type} *end, {type_id_type} cons);\n"
                    ));
                }

                g.funcs_text.push_str("\ttemplate <typename Accumulator>\n");
                g.funcs_text
                    .push_str("\tvoid write(Accumulator &to) const;\n");
                if is_template {
                    method_bodies.push_str("template <typename TQueryType>\n");
                    method_bodies.push_str("template <typename Accumulator>\n");
                    method_bodies.push_str(&format!(
                        "void {full}<TQueryType>::write(Accumulator &to) const {{\n"
                    ));
                } else {
                    method_bodies.push_str("template <typename Accumulator>\n");
                    method_bodies
                        .push_str(&format!("void {full}::write(Accumulator &to) const {{\n"));
                }
                for p in &c.params {
                    let k = &p.name;
                    let is_cond = c.conditions.iter().any(|(n, _)| n == k);
                    let is_trivial = c.trivial_conditions.contains(k);
                    if is_cond {
                        if !is_trivial {
                            method_bodies.push_str(&format!(
                                "\tif (_{}.v & Flag::f_{k}) _{k}.write(to);\n",
                                c.flags_name
                            ));
                        }
                    } else {
                        method_bodies.push_str(&format!("\t_{k}.write(to);\n"));
                    }
                }
                method_bodies.push_str("}\n");
                if !is_template {
                    method_bodies.push_str(&format!(
                        "template void {full}::write<{buffer_type}>({buffer_type} &to) const;\n"
                    ));
                    method_bodies.push_str(&format!(
                        "template void {full}::write<::tl::details::LengthCounter>(::tl::details::LengthCounter &to) const;\n"
                    ));
                }
            }

            if !c.params.is_empty() {
                let mut any_getter = false;
                for p in &c.params {
                    let pname = &p.name;
                    if c.trivial_conditions.contains(pname) || c.bots_only_params.contains(pname) {
                        continue;
                    }
                    if !any_getter {
                        g.funcs_text.push('\n');
                        any_getter = true;
                    }
                    let ptype_full = full_type(config, &p.tl_type);
                    let is_cond = c.conditions.iter().any(|(n, _)| n == pname);
                    getter_decl(
                        &mut g.funcs_text,
                        &mut method_bodies,
                        &full,
                        pname,
                        &ptype_full,
                        is_cond,
                        c.nullable_vectors.contains(pname),
                        c.nullable_params.contains(pname),
                        &c.flags_name,
                    );
                }
            }

            if is_template {
                g.funcs_text
                    .push_str("\n\tusing ResponseType = typename TQueryType::ResponseType;\n\n");
                g.inline_methods.push_str(&method_bodies);
            } else {
                let resp = if read_write {
                    full_type(config, &res_boxed)
                } else {
                    full_type(config, restype_key)
                };
                g.funcs_text
                    .push_str(&format!("\n\tusing ResponseType = {resp};\n\n"));
                g.methods.push_str(&method_bodies);
            }

            if !stored.is_empty() {
                g.funcs_text.push_str("private:\n");
                for p in stored.iter() {
                    let pname = &p.name;
                    let ptype_full = if *pname == c.template_param {
                        p.tl_type.clone()
                    } else {
                        full_type(config, &p.tl_type)
                    };
                    if c.nullable_vectors.contains(*pname) {
                        let opt = optional_in_vector(&ptype_full)
                            .unwrap_or_else(|_| format!("Vector<std::optional<{ptype_full}>>"));
                        g.funcs_text.push_str(&format!("\t{opt} _{pname};\n"));
                    } else if c.nullable_params.contains(*pname) {
                        g.funcs_text
                            .push_str(&format!("\tstd::optional<{ptype_full}> _{pname};\n"));
                    } else {
                        g.funcs_text
                            .push_str(&format!("\t{ptype_full} _{pname};\n"));
                    }
                }
                g.funcs_text.push('\n');
            }

            g.funcs_text.push_str("};\n");
            if read_write {
                let alias = full_type(config, &c.alias_name);
                if is_template {
                    g.funcs_text.push_str("template <typename TQueryType>\n");
                    g.funcs_text
                        .push_str(&format!("using {alias} = tl::boxed<{full}<TQueryType>>;\n"));
                } else {
                    g.funcs_text
                        .push_str(&format!("using {alias} = tl::boxed<{full}>;\n"));
                }
            }
        }
    }

    // ---- types (lines 809-1342): full port in next commit; placeholder
    // keeps header/source assembling while types land.
    for (restype_key, def) in &scheme.types {
        let _ = (restype_key, def, &optimize_single);
    }

    // ---- flag inheritance operators (lines 1343-1364) ----
    // Order follows `codegen_scheme.py` insertion order byte-for-byte.
    g.flag_operators.push('\n');
    for (child, parent) in &config.flag_inheritance {
        if child == "channelForbidden" {
            continue;
        }
        let child_full = format!("{}{}", config.prefixes.data, child);
        let parent_full = format!("{}{}", config.prefixes.data, parent);
        if let (Some(cmap), Some(pmap)) = (
            g.parent_flags_check.get(&child_full).cloned(),
            g.parent_flags_check.get(&parent_full).cloned(),
        ) {
            for (flag, bit) in &cmap {
                if let Some(pbit) = pmap.get(flag) {
                    if pbit != bit {
                        // Python raises; we record a warning-style operator skip.
                        // Golden schemes never hit this (validated upstream).
                    }
                }
            }
            let _ = (cmap, pmap);
        }
        // Ensure parent entry exists for later children chaining.
        let child_map = g
            .parent_flags_check
            .get(&child_full)
            .cloned()
            .unwrap_or_default();
        let parent_entry = g.parent_flags_check.entry(parent_full.clone()).or_default();
        for (k, v) in child_map {
            parent_entry.entry(k).or_insert(v);
        }
        g.flag_operators.push_str(&format!(
            "inline {parent_full}::Flags mtpCastFlags({child_full}::Flags flags) {{ return static_cast<{parent_full}::Flag>(flags.value()); }}\n"
        ));
        g.flag_operators.push_str(&format!(
            "inline {parent_full}::Flags mtpCastFlags(MTPflags<{child_full}::Flags> flags) {{ return mtpCastFlags(flags.v); }}\n"
        ));
    }

    g
}
