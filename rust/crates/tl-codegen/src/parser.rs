//! Main parse loop: port of `readAndGenerate` lines 340-794 (everything
//! before the emitters). Turns `TlInputs` lines into a populated [`Scheme`].
//!
//! Control flow mirrors the Python line-for-line: comment split, section
//! switches, skip/bots/mobile filters, constructor regex, rename + CRC
//! verify, result-type normalize, per-param loop, table insertion. The only
//! deliberate difference: Python `raise`s on malformed input; we collect
//! `warnings` and skip the line, because a codegen tool must report all
//! errors in one run, not die on the first.

use crate::config::CodegenScheme;
use crate::crc;
use crate::ir::{Constructor, Param, Scheme, TypeDef};
use crate::parse::{
    normalized_bare_name, normalized_name, optional_in_vector, parse_ctor_header, parse_raw_param,
    RawParam,
};
use crate::resolve::Resolver;
use crate::tl;
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

fn comment_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(.*?)//(.*?)$").unwrap())
}

fn section_re(s: &str) -> bool {
    s.contains("---functions---") || s.contains("---types---")
}

/// `cleanline` reconstruction (lines 421-431): the exact string the CRC is
/// computed over. Kept separate so golden tests can assert it directly.
pub fn clean_line_for_crc(
    ctor_name: &str,
    params_raw: &str,
    result: &str,
    scheme: &CodegenScheme,
) -> String {
    // `cleanline = group(1) + group(3) + '= ' + group(4)`.
    let mut line = format!("{ctor_name}{params_raw}= {result}");
    // Strip ` name:flags2?.N?true` optional-true params.
    static TRUE_RE: OnceLock<Regex> = OnceLock::new();
    let true_re =
        TRUE_RE.get_or_init(|| Regex::new(r" [a-zA-Z0-9_]+\:flags2?\.[0-9]+\?true").unwrap());
    line = true_re.replace_all(&line, "").into_owned();
    line = line.replace('<', " ").replace('>', " ");
    // Single non-overlapping pass, exactly like Python `str.replace`.
    line = line.replace("  ", " ");
    line = line
        .strip_prefix(' ')
        .unwrap_or(&line)
        .strip_suffix(' ')
        .unwrap_or(&line)
        .to_string();
    for (from, to) in &scheme.synonyms {
        line = line.replace(&format!(":{from} "), &format!(":{to} "));
        line = line.replace(&format!("?{from} "), &format!("?{to} "));
    }
    line = line.replace('{', "").replace('}', "");
    line
}

/// Normalize a result type exactly like lines 450-463, returning
/// `(dict_key, boxed_alias)`: `dict_key` is the lowercased-first `restype`
/// used as the `typesDict`/`funcsDict` key (e.g. `Bool` -> `bool`,
/// `ns.Res` -> `ns_res`); `boxed_alias` is the case-preserving `resType`
/// stored in `TypesDict` (e.g. `Bool`, `ns_Res`).
fn normalize_result(
    restype_raw: &str,
    resolver: &Resolver<'_>,
    scheme_parsed: &Scheme,
) -> Result<(String, String), String> {
    let mut restype = restype_raw.to_string();
    if restype.contains('<') {
        restype =
            resolver.handle_template(&restype, scheme_parsed, &|rr, n| Ok(rr.full_type_name(n)))?;
    }
    let res_type = normalized_name(&restype);
    let key: String;
    if restype.contains('.') {
        // `ns.Res` -> key `ns_res`, boxed `ns_Res`.
        let dot = restype.rfind('.').unwrap();
        let (ns, tail) = restype.split_at(dot);
        let tail = &tail[1..];
        let first = tail
            .chars()
            .next()
            .ok_or_else(|| format!("Bad result type name with dot: {restype}"))?;
        if !first.is_ascii_uppercase() {
            return Err(format!("Bad result type name with dot: {restype}"));
        }
        key = format!(
            "{}_{}{}",
            ns.replace('.', "_"),
            first.to_ascii_lowercase(),
            &tail[1..]
        );
    } else if restype
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
    {
        let mut b = restype.clone();
        b.replace_range(..1, &restype[..1].to_lowercase());
        key = b;
    } else {
        return Err(format!("Bad result type name: {restype}"));
    }
    Ok((key, res_type))
}

/// Full parse loop. Returns a populated `Scheme` (types + funcs tables,
/// enums, warnings).
pub fn parse_inputs(inputs: &tl::TlInputs, scheme: &CodegenScheme) -> Scheme {
    let mut out = Scheme {
        layer: inputs.layer,
        input_names: inputs.names.clone(),
        ..Scheme::default()
    };
    let resolver = Resolver::new(scheme);
    let read_write = scheme.read_write_section();
    let mut funcs_now = false;
    let mut accumulated = String::new();
    // `TypeConstructors[normalized] = {typeBare, typeBoxed}` — needed for
    // the `prms[pname]` resolution at line 559-562.
    let mut type_ctors: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    for raw in &inputs.lines {
        let mut line = raw.clone();
        let mut comment = String::new();
        let nocomment = comment_re().captures(&line).map(|c| {
            (
                c.get(1).unwrap().as_str().to_string(),
                c.get(2).unwrap().as_str().to_string(),
            )
        });
        if let Some((code, cmt)) = nocomment.clone() {
            line = code;
            comment = cmt;
        }
        let is_comment_line = nocomment.is_some();
        if line.contains("---functions---") {
            funcs_now = true;
            continue;
        }
        if line.contains("---types---") {
            funcs_now = false;
            continue;
        }
        if line.trim().is_empty() {
            if !is_comment_line {
                accumulated.clear();
            } else if !comment.is_empty() {
                accumulated.push(' ');
                accumulated.push_str(&comment);
            }
            continue;
        }
        if scheme.skip.contains(line.trim()) {
            continue;
        }
        let _ = section_re(&line);

        let header = match parse_ctor_header(&line) {
            Some(h) => h,
            None => {
                out.warnings.push(format!("Bad line found: {line}"));
                continue;
            }
        };
        let comments = std::mem::take(&mut accumulated);

        if tl::is_bots_only_line(&comments) || tl::is_mobile_only_line(&comments) {
            continue;
        }

        // Rename + `Name` mangling (lines 405-414).
        let original = header.name.clone();
        let mut name = original.clone();
        if let Some(renamed) = scheme.renamed_types.get(&name) {
            name = renamed.clone();
        }
        let class_name: String;
        if let Some(ind) = name.rfind('.') {
            let (ns, tail) = name.split_at(ind);
            let tail = &tail[1..];
            let mut up = tail.to_string();
            up.replace_range(..1, &tail[..1].to_uppercase());
            class_name = format!("{ns}_{up}");
            name = normalized_name(&name);
        } else {
            let mut up = name.clone();
            up.replace_range(..1, &name[..1].to_uppercase());
            class_name = up;
        }

        // CRC verify (lines 415-446).
        let mut typeid_hex = header.id_hex.clone().unwrap_or_default();
        while typeid_hex.starts_with('0') && !typeid_hex.is_empty() {
            typeid_hex.remove(0);
        }
        let cleaned = clean_line_for_crc(&header.name, &header.params_raw, &header.result, scheme);
        let mut counted = crc::format_id(crc::type_id(&cleaned));
        while counted.starts_with('0') && !counted.is_empty() {
            counted.remove(0);
        }
        let type_id: u32;
        if !typeid_hex.is_empty() {
            if typeid_hex != counted {
                let key = format!("{original}#{typeid_hex}");
                if !scheme.type_id_exceptions.contains(&key) {
                    out.warnings.push(format!(
                        "Warning: counted {counted} mismatch with provided {typeid_hex} ({key}, {cleaned})"
                    ));
                    continue;
                }
                type_id = u32::from_str_radix(&typeid_hex, 16).unwrap_or(0);
            } else {
                type_id =
                    u32::from_str_radix(&typeid_hex, 16).unwrap_or_else(|_| crc::type_id(&cleaned));
            }
        } else {
            type_id = crc::type_id(&cleaned);
        }

        // Result-type normalize (lines 448-466).
        let (res_key, boxed_alias) = match normalize_result(&header.result, &resolver, &out) {
            Ok(t) => t,
            Err(e) => {
                out.warnings.push(e);
                continue;
            }
        };

        let full_name = resolver.full_type_name(&name);
        let full_data_name = resolver.full_data_name(&name);
        let bare_name = normalized_bare_name(&name).unwrap_or_else(|_| name.clone());
        let enum_line = format!(
            "\t{} = {:#x}",
            format!("{}{}", scheme.id_prefix(), name),
            type_id
        );

        // Param loop (lines 470-562).
        let mut params: Vec<Param> = Vec::new();
        let mut prms_list: Vec<String> = Vec::new();
        let mut prms: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut conditions: Vec<(String, u32)> = Vec::new();
        let mut cond_seen: HashSet<String> = HashSet::new();
        let mut trivial: HashSet<String> = HashSet::new();
        let mut has_flags = String::new();
        let mut has_flags64 = String::new();
        let mut has_template = String::new();
        let mut is_template = String::new();
        let mut nullable_params: HashSet<String> = HashSet::new();
        let mut nullable_vectors: HashSet<String> = HashSet::new();
        let mut bots_only: HashSet<String> = HashSet::new();
        let mut failed = false;

        for token in header.params_raw.trim().split(' ') {
            if token.trim().is_empty() {
                continue;
            }
            // `{X:Type}` template placeholder (line 483).
            if token.starts_with('{') && token.ends_with('}') {
                let inner = &token[1..token.len() - 1];
                if let Some((var, bound)) = inner.split_once(':') {
                    if bound == "Type" && var.chars().all(|c| c.is_ascii_alphabetic()) {
                        has_template = var.to_string();
                        continue;
                    }
                }
                out.warnings
                    .push(format!("Bad param found: \"{token}\" in line: {line}"));
                failed = true;
                break;
            }
            let rp = match parse_raw_param(token) {
                Some(p) => p,
                None => {
                    out.warnings
                        .push(format!("Bad param found: \"{token}\" in line: {line}"));
                    failed = true;
                    break;
                }
            };
            match rp {
                RawParam::TemplateDecl { var, bound } => {
                    if bound == "Type" {
                        has_template = var;
                    } else {
                        out.warnings
                            .push(format!("Bad param found: \"{token}\" in line: {line}"));
                        failed = true;
                        break;
                    }
                }
                RawParam::TemplateUse(var) => {
                    let pname = token.split(':').next().unwrap_or("").to_string();
                    if format!("!{has_template}") == format!("!{var}") && !has_template.is_empty() {
                        let _ = &pname;
                        // `isTemplate = pname`, type `TQueryType`.
                        let pname = token.split(':').next().unwrap_or("").to_string();
                        is_template = pname.clone();
                        prms_list.push(pname.clone());
                        prms.insert(pname.clone(), "TQueryType".to_string());
                        params.push(Param {
                            name: pname,
                            tl_type: "TQueryType".to_string(),
                            raw_tl_type: token.to_string(),
                            flag_bit: None,
                            is_true_flag: false,
                        });
                    } else {
                        out.warnings.push(format!(
                            "Bad template param name: \"{token}\" in line: {line}"
                        ));
                        failed = true;
                        break;
                    }
                }
                RawParam::Flags => {
                    let pname = token.split(':').next().unwrap_or("").to_string();
                    if !has_flags.is_empty() && pname == format!("{has_flags}2") {
                        has_flags64 = pname;
                        continue;
                    }
                    has_flags = pname.clone();
                    let ptype = if funcs_now {
                        format!("flags<{}::Flags>", resolver.full_type_name(&name))
                    } else {
                        format!("flags<{}::Flags>", resolver.full_data_name(&name))
                    };
                    prms_list.push(pname.clone());
                    prms.insert(pname.clone(), ptype.clone());
                    params.push(Param {
                        name: pname,
                        tl_type: ptype,
                        raw_tl_type: token.to_string(),
                        flag_bit: None,
                        is_true_flag: false,
                    });
                }
                RawParam::Plain { name: pname, ty } => {
                    let bots_prm = tl::is_bots_only_param(&comments, &pname);
                    let nul_vec = !bots_prm && tl::is_nullable_vector(&comments, &pname);
                    let nul_prm = !bots_prm && !nul_vec && tl::is_nullable_param(&comments, &pname);
                    if bots_prm {
                        bots_only.insert(pname.clone());
                    }
                    let mut ptype = ty.clone();
                    if ptype.contains('<') {
                        if nul_prm {
                            out.warnings.push(format!(
                                "Vector param should not be nullable: \"{pname}:{ptytype}\" in line: {line}",
                                ptytype = ty
                            ));
                            failed = true;
                            break;
                        }
                        if nul_vec {
                            nullable_vectors.insert(pname.clone());
                        }
                        let resolved = if read_write {
                            resolver.handle_template(
                                &ptype,
                                &out,
                                &|rr, n| Ok(rr.full_type_name(n)),
                            )
                        } else {
                            resolver
                                .handle_template(&ptype, &out, &|rr, n| rr.full_bare_type_name(n))
                        };
                        match resolved {
                            Ok(r) => ptype = r,
                            Err(e) => {
                                out.warnings.push(e);
                                failed = true;
                                break;
                            }
                        }
                    } else if nul_vec {
                        out.warnings.push(format!(
                            "Non-vector param should not be vector-nullable: \"{pname}:{ptytype}\" in line: {line}",
                            ptytype = ty
                        ));
                        failed = true;
                        break;
                    } else if nul_prm {
                        nullable_params.insert(pname.clone());
                    }
                    prms_list.push(pname.clone());
                    let normalized = if read_write {
                        normalized_name(&ptype)
                    } else {
                        normalized_bare_name(&ptype).unwrap_or_else(|_| ptype.clone())
                    };
                    let resolved = match type_ctors.get(&normalized) {
                        Some((bare, _)) => bare.clone(),
                        None => normalized,
                    };
                    prms.insert(pname.clone(), resolved.clone());
                    params.push(Param {
                        name: pname,
                        tl_type: resolved,
                        raw_tl_type: ptype,
                        flag_bit: None,
                        is_true_flag: false,
                    });
                }
                RawParam::Flagged {
                    name: pname,
                    flag,
                    bit,
                    inner,
                } => {
                    if flag != has_flags && flag != has_flags64 {
                        out.warnings
                            .push(format!("Bad param found: \"{token}\" in line: {line}"));
                        failed = true;
                        break;
                    }
                    let mut ptype = inner.clone();
                    if ptype.contains('<') {
                        // `handleTemplate(ptype)` or with `fullBareTypeName`.
                        let resolved = if read_write {
                            resolver.handle_template(
                                &ptype,
                                &out,
                                &|rr, n| Ok(rr.full_type_name(n)),
                            )
                        } else {
                            resolver
                                .handle_template(&ptype, &out, &|rr, n| rr.full_bare_type_name(n))
                        };
                        match resolved {
                            Ok(r) => ptype = r,
                            Err(e) => {
                                out.warnings.push(e);
                                failed = true;
                                break;
                            }
                        }
                    }
                    let bit_val = if flag == has_flags64 { bit + 32 } else { bit };
                    if !cond_seen.contains(&pname) {
                        cond_seen.insert(pname.clone());
                        conditions.push((pname.clone(), bit_val));
                        if ptype == "true" {
                            trivial.insert(pname.clone());
                        }
                    }
                    prms_list.push(pname.clone());
                    let normalized = if read_write {
                        normalized_name(&ptype)
                    } else {
                        normalized_bare_name(&ptype).unwrap_or_else(|_| ptype.clone())
                    };
                    let resolved = match type_ctors.get(&normalized) {
                        Some((bare, _)) => bare.clone(),
                        None => normalized,
                    };
                    prms.insert(pname.clone(), resolved.clone());
                    params.push(Param {
                        name: pname,
                        tl_type: resolved,
                        raw_tl_type: ptype,
                        flag_bit: Some(bit_val),
                        is_true_flag: trivial
                            .contains(&params.last().map(|_| String::new()).unwrap_or_default()),
                    });
                    if let Some(last) = params.last_mut() {
                        last.is_true_flag = trivial.contains(&last.name);
                    }
                }
            }
        }
        // Python pushes the enum line BEFORE the param loop, so even a
        // constructor that later fails still leaves its enum constant.
        out.enums.push(enum_line);

        if failed {
            continue;
        }

        if is_template.is_empty() && res_key == "X" {
            out.warnings.push(format!(
                "Bad response type \"X\" in \"{name}\" in line: {line}"
            ));
            continue;
        }

        let ctor = Constructor {
            name: name.clone(),
            tl_name: original.clone(),
            alias_name: class_name.clone(),
            bare_name,
            full_name,
            full_data_name,
            type_id,
            params,
            has_flags: !has_flags.is_empty(),
            has_flags64: !has_flags64.is_empty(),
            flags_name: has_flags.clone(),
            flags64_name: has_flags64.clone(),
            template_param: is_template.clone(),
            template_var: has_template.clone(),
            conditions,
            trivial_conditions: trivial,
            is_template: !is_template.is_empty(),
            has_template: !has_template.is_empty(),
            nullable_params,
            nullable_vectors,
            bots_only_params: bots_only,
        };
        let _ = (class_name, boxed_alias.clone(), prms, prms_list);

        if funcs_now {
            if ctor.is_template {
                // Template funcs allowed; template *types* rejected below.
            }
            let table = out.funcs.entry(res_key.clone()).or_insert(TypeDef {
                restype: res_key.clone(),
                boxed_name: boxed_alias.clone(),
                ..TypeDef::default()
            });
            table.boxed_name = boxed_alias.clone();
            table.ctors.push(ctor);
        } else {
            if ctor.is_template {
                out.warnings.push(format!(
                    "Template types not allowed: \"{res_key}\" in line: {line}"
                ));
                continue;
            }
            let table = out.types.entry(res_key.clone()).or_insert(TypeDef {
                restype: res_key.clone(),
                boxed_name: boxed_alias.clone(),
                ..TypeDef::default()
            });
            table.boxed_name = boxed_alias.clone();
            table.ctors.push(ctor);
            type_ctors.insert(name.clone(), (res_key.clone(), boxed_alias));
        }
    }

    // Post-pass: `withType = len > 1`, `withData`, `nullable`.
    // `withData` is set per-constructor in Python: any constructor whose
    // stored params (`len(prms) > len(trivialConditions) + len(botsOnlyPrms)`)
    // is non-empty marks the whole type as having data.
    for (key, def) in out.types.iter_mut() {
        def.with_type = def.ctors.len() > 1;
        def.with_data = def.ctors.iter().any(|c| {
            c.params
                .iter()
                .filter(|p| !c.trivial_conditions.contains(&p.name))
                .filter(|p| !c.bots_only_params.contains(&p.name))
                .count()
                > 0
        });
        def.nullable = scheme.nullable.contains(key);
    }
    for (key, def) in out.funcs.iter_mut() {
        def.with_type = def.ctors.len() > 1;
        def.with_data = def.ctors.iter().any(|c| {
            c.params
                .iter()
                .filter(|p| !c.trivial_conditions.contains(&p.name))
                .filter(|p| !c.bots_only_params.contains(&p.name))
                .count()
                > 0
        });
        def.nullable = scheme.nullable.contains(key);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_scheme() -> CodegenScheme {
        serde_json::from_value(serde_json::json!({
            "prefixes": { "type": "MTP", "data": "MTPD", "id": "mtpc", "construct": "MTP_" },
            "types": { "prime": "mtpPrime", "typeId": "mtpTypeId", "buffer": "mtpBuffer" },
            "sections": ["read-write"],
            "builtin": ["int", "long", "double", "string", "bytes", "int128", "int256"],
            "builtinTemplates": ["vector", "flags"],
            "synonyms": { "bytes": "string" },
        }))
        .unwrap()
    }

    #[test]
    fn parses_real_constructors() {
        let scheme = test_scheme();
        let inputs = tl::TlInputs {
            lines: vec![
                "---types---".to_string(),
                "boolFalse#bc799737 = Bool;".to_string(),
                "boolTrue#997275b5 = Bool;".to_string(),
                "inputPeerChannel#27bcbbfc channel_id:long access_hash:long = InputPeer;"
                    .to_string(),
                "inputPhoneContact#6a1dc4be flags:# client_id:long phone:string note:flags.0?TextWithEntities = InputContact;".to_string(),
            ],
            layer: 229,
            names: vec!["test.tl".to_string()],
        };
        let parsed = parse_inputs(&inputs, &scheme);
        assert_eq!(parsed.layer, 229);
        assert_eq!(parsed.types.len(), 3); // Bool, InputPeer, InputContact
        let bool_def = &parsed.types["bool"];
        assert_eq!(bool_def.ctors.len(), 2);
        assert!(bool_def.with_type);
        let contact = &parsed.types["input_contact"];
        assert_eq!(contact.ctors.len(), 1);
        let c = &contact.ctors[0];
        assert!(c.has_flags);
        assert_eq!(c.conditions, vec![("note".to_string(), 0)]);
        assert_eq!(parsed.warnings, Vec::<String>::new());
    }

    #[test]
    fn crc_mismatch_warns_and_skips() {
        let scheme = test_scheme();
        let inputs = tl::TlInputs {
            lines: vec![
                "---types---".to_string(),
                "boolFalse#deadbeef = Bool;".to_string(),
            ],
            layer: 1,
            names: vec!["test.tl".to_string()],
        };
        let parsed = parse_inputs(&inputs, &scheme);
        assert!(parsed.types.is_empty());
        assert_eq!(parsed.warnings.len(), 1);
        assert!(parsed.warnings[0].contains("mismatch"));
    }
}
