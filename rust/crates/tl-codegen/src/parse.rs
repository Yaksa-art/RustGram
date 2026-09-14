//! Constructor + parameter parsing: port of the main-loop regexes
//! (generate_tl.py lines 395, 487) and the name/template helpers
//! (lines 238-283).
//!
//! The two regexes are ported verbatim — their quirks (what they accept
//! and reject) are load-bearing for byte-identical output.

use regex::Regex;
use std::sync::OnceLock;

struct Patterns {
    ctor: Regex,
    param: Regex,
    dotted_box: Regex,
    vector_tpl: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        // `([a-zA-Z\.0-9_]+)(#[0-9a-f]+)?([^=]*)=\s*([a-zA-Z\.<>0-9_]+);`
        ctor: Regex::new(
            r"([a-zA-Z\.0-9_]+)(#[0-9a-f]+)?([^=]*)=\s*([a-zA-Z\.<>0-9_]+);",
        )
        .unwrap(),
        // `([a-zA-Z_][a-zA-Z0-9_]*):([A-Za-z0-9<>\._]+\|![a-zA-Z]+\|#\|[a-z_][a-z0-9_]*\.[0-9]+\?[A-Za-z0-9<>\._]+)$`
        param: Regex::new(
            r"([a-zA-Z_][a-zA-Z0-9_]*):([A-Za-z0-9<>\._]+|![a-zA-Z]+|#|[a-z_][a-z0-9_]*\.[0-9]+\?[A-Za-z0-9<>\._]+)$",
        )
        .unwrap(),
        dotted_box: Regex::new(r"^([a-zA-Z0-9])+\.([A-Z][a-zA-Z0-9]+)$").unwrap(),
        vector_tpl: Regex::new(r"(.*?)([vV]ector<)([A-Za-z0-9_]+)>$").unwrap(),
    })
}

/// `normalizedName`: `a.b` -> `a_b`.
pub fn normalized_name(name: &str) -> String {
    name.replace('.', "_")
}

/// `normalizedBareName`: `ns.Box` -> `ns_box`; bare `Box` -> `box`.
/// Raises (returns `Err`) on `Bad name` exactly like Python.
pub fn normalized_bare_name(name: &str) -> Result<String, String> {
    if let Some(c) = patterns().dotted_box.captures(name) {
        let head = c.get(1).unwrap().as_str();
        let tail = c.get(2).unwrap().as_str();
        let mut lower = tail.to_string();
        lower.replace_range(..1, &tail[..1].to_lowercase());
        return Ok(format!("{head}_{lower}"));
    }
    if name.contains('.') {
        return Err(format!("Bad name: {name}"));
    }
    let mut out = name.to_string();
    out.replace_range(..1, &name[..1].to_lowercase());
    Ok(out)
}

/// Parsed constructor header: name, optional `#hexid`, raw params, result.
#[derive(Debug)]
pub struct CtorHeader {
    pub name: String,
    pub id_hex: Option<String>,
    pub params_raw: String,
    pub result: String,
}

/// Apply the constructor regex to one (comment-stripped) line.
pub fn parse_ctor_header(line: &str) -> Option<CtorHeader> {
    let c = patterns().ctor.captures(line.trim())?;
    Some(CtorHeader {
        name: c.get(1)?.as_str().to_string(),
        id_hex: c
            .get(2)
            .map(|m| m.as_str().trim_start_matches('#').to_string()),
        params_raw: c.get(3).unwrap().as_str().to_string(),
        result: c.get(4)?.as_str().to_string(),
    })
}

/// One raw parameter after `params.strip().split(' ')`.
#[derive(Debug, Clone, PartialEq)]
pub enum RawParam {
    /// `{X:Type}` template placeholder.
    TemplateDecl { var: String, bound: String },
    /// `!X` template use.
    TemplateUse(String),
    /// `name:#` flags field.
    Flags,
    /// `name:flags.N?Type` conditional.
    Flagged {
        name: String,
        flag: String,
        bit: u32,
        inner: String,
    },
    /// Plain `name:Type`.
    Plain { name: String, ty: String },
}

/// Apply the param regex to one token. Returns `None` if it does not match
/// (Python would `raise` downstream — we surface it as an error instead).
pub fn parse_raw_param(token: &str) -> Option<RawParam> {
    // `{X:Type}` never matches the param regex; handle first like Python's
    // `hasTemplate` branch.
    if let Some(inner) = token.strip_prefix('{').and_then(|t| t.strip_suffix('}')) {
        let (var, bound) = inner.split_once(':')?;
        return Some(RawParam::TemplateDecl {
            var: var.to_string(),
            bound: bound.to_string(),
        });
    }
    let c = patterns().param.captures(token)?;
    let name = c.get(1)?.as_str().to_string();
    let ty = c.get(2)?.as_str().to_string();
    if let Some(var) = ty.strip_prefix('!') {
        return Some(RawParam::TemplateUse(var.to_string()));
    }
    if ty == "#" {
        return Some(RawParam::Flags);
    }
    if let Some((flag, rest)) = ty.split_once('.') {
        if let Some((bit_str, inner)) = rest.split_once('?') {
            if let Ok(bit) = bit_str.parse::<u32>() {
                return Some(RawParam::Flagged {
                    name,
                    flag: flag.to_string(),
                    bit,
                    inner: inner.to_string(),
                });
            }
        }
    }
    Some(RawParam::Plain { name, ty })
}

/// `optionalInVector`: `Vector<T>` -> `Vector<optional<T>>`.
pub fn optional_in_vector(name: &str) -> Result<String, String> {
    match patterns().vector_tpl.captures(name) {
        Some(c) => Ok(format!(
            "{}{}std::optional<{}>>",
            c.get(1).unwrap().as_str(),
            c.get(2).unwrap().as_str(),
            c.get(3).unwrap().as_str()
        )),
        None => Err(format!("Bad optional vector: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctor_regex_matches_real_lines() {
        let h = parse_ctor_header(
            "inputPeerChannel#27bcbbfc channel_id:long access_hash:long = InputPeer;",
        )
        .unwrap();
        assert_eq!(h.name, "inputPeerChannel");
        assert_eq!(h.id_hex.as_deref(), Some("27bcbbfc"));
        assert_eq!(h.result, "InputPeer");

        // No-id constructor: `tlsBlockDomain = TlsBlock;`
        let bare = parse_ctor_header("tlsBlockDomain = TlsBlock;").unwrap();
        assert_eq!(bare.id_hex, None);
    }

    #[test]
    fn flags_and_vectors() {
        assert_eq!(parse_raw_param("flags:#"), Some(RawParam::Flags));
        assert_eq!(
            parse_raw_param("spoiler:flags.2?true"),
            Some(RawParam::Flagged {
                name: "spoiler".into(),
                flag: "flags".into(),
                bit: 2,
                inner: "true".into()
            })
        );
        assert_eq!(
            parse_raw_param("file:Vector<InputDocument>"),
            Some(RawParam::Plain {
                name: "file".into(),
                ty: "Vector<InputDocument>".into()
            })
        );
    }

    #[test]
    fn bare_names() {
        assert_eq!(normalized_bare_name("Box").unwrap(), "box");
        assert_eq!(normalized_bare_name("ns.Box").unwrap(), "ns_box");
        assert!(normalized_bare_name("a.b.c").is_err());
    }
}
