//! Name resolution: `fullTypeName` / `fullBareTypeName` / `fullDataName`
//! (lines 247-252) and `handleTemplate` (lines 259-283).
//!
//! These need the parsed type tables (for the `foundmeta` fallback), so they
//! live behind a small `Resolver` instead of free functions.

use crate::config::CodegenScheme;
use crate::ir::Scheme;
use crate::parse::{normalized_bare_name, normalized_name};

/// Prefix application helpers bound to one scheme config.
pub struct Resolver<'a> {
    pub scheme: &'a CodegenScheme,
}

impl<'a> Resolver<'a> {
    pub fn new(scheme: &'a CodegenScheme) -> Self {
        Self { scheme }
    }

    pub fn full_type_name(&self, name: &str) -> String {
        format!("{}{}", self.scheme.prefixes.type_, normalized_name(name))
    }

    pub fn full_bare_type_name(&self, name: &str) -> Result<String, String> {
        Ok(format!(
            "{}{}",
            self.scheme.prefixes.type_,
            normalized_bare_name(name)?
        ))
    }

    pub fn full_data_name(&self, name: &str) -> String {
        format!("{}{}", self.scheme.prefixes.data, normalized_name(name))
    }

    /// `handleTemplate(name, process)`: resolve `Vector<X>` recursively.
    /// `process` is one of the `full*` mappers above; the default is
    /// `fullTypeName`.
    pub fn handle_template(
        &self,
        name: &str,
        scheme_parsed: &Scheme,
        process: &dyn Fn(&Self, &str) -> Result<String, String>,
    ) -> Result<String, String> {
        let name = name.trim();
        let (head, tail) = match split_outer_template(name) {
            Some(t) => t,
            None => return Err(format!("Bad template type: {name}")),
        };
        let inner = tail;
        let resolved_inner = if inner.contains('<') {
            let nested = self.handle_template(inner, scheme_parsed, process)?;
            process(self, &nested)?
        } else if starts_uppercase(inner)
            || is_dotted_upper(inner)
            || self.scheme.is_builtin_type(inner)
        {
            process(self, inner)?
        } else {
            // `foundmeta` fallback: search parsed tables for a constructor
            // whose normalized name matches.
            match find_meta(scheme_parsed, inner) {
                Some(meta) => process(self, &meta)?,
                None => return Err(format!("Bad vector param: {inner}")),
            }
        };
        Ok(format!("{head}<{resolved_inner}>"))
    }
}

/// Split `Vector<...>` / `vector<...>` into head + inner, respecting nesting.
fn split_outer_template(name: &str) -> Option<(&str, &str)> {
    let lt = name.find('<')?;
    if !name.ends_with('>') {
        return None;
    }
    let head = &name[..lt + 1]; // keeps `<`
    let head_name = &head[..lt];
    if head_name != "Vector" && head_name != "vector" {
        return None;
    }
    // Balance check for nesting.
    let mut depth = 0;
    for (i, ch) in name.char_indices() {
        if ch == '<' {
            depth += 1;
        } else if ch == '>' {
            depth -= 1;
            if depth == 0 && i != name.len() - 1 {
                return None;
            }
        }
    }
    if depth != 0 {
        return None;
    }
    Some((&name[..lt], &name[lt + 1..name.len() - 1]))
}

fn starts_uppercase(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

fn is_dotted_upper(s: &str) -> bool {
    match s.split_once('.') {
        Some((_, tail)) => tail.chars().next().is_some_and(|c| c.is_ascii_uppercase()),
        None => false,
    }
}

/// Search parsed type tables for a normalized constructor name.
fn find_meta(scheme: &Scheme, vectemplate: &str) -> Option<String> {
    let want = normalized_name(vectemplate);
    for table in [&scheme.types, &scheme.funcs] {
        for (metatype, def) in table {
            if def.ctors.iter().any(|t| t.name == want) {
                return Some(metatype.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme() -> CodegenScheme {
        serde_json::from_value(serde_json::json!({
            "prefixes": { "type": "MTP", "data": "MTPD", "id": "mtpc", "construct": "MTP_" },
            "builtin": ["int", "long"],
            "builtinTemplates": ["vector", "flags"],
        }))
        .unwrap()
    }

    #[test]
    fn prefixes() {
        let s = scheme();
        let r = Resolver::new(&s);
        assert_eq!(r.full_type_name("ns.cons"), "MTPns_cons");
        assert_eq!(r.full_data_name("ns.cons"), "MTPDns_cons");
        assert_eq!(r.full_bare_type_name("Box").unwrap(), "MTPbox");
    }

    #[test]
    fn builtin_vector() {
        let s = scheme();
        let r = Resolver::new(&s);
        let parsed = Scheme::default();
        let out = r
            .handle_template("Vector<int>", &parsed, &|rr, n| Ok(rr.full_type_name(n)))
            .unwrap();
        assert_eq!(out, "Vector<MTPint>");
    }
}
