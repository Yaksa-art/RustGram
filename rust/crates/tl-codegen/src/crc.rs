//! CRC-32 type-id verification: port of generate_tl.py lines 421-444.
//!
//! For each constructor the generator rebuilds a `cleanline` from the
//! declaration (stripping `name:flags.N?true` params, `<>` to spaces,
//! synonyms, `{}`), runs `crc32` over it, and compares against the `#hexid`
//! written in the `.tl` (leading `0`s stripped). Mismatch — unless the line
//! is in `typeIdExceptions` — prints `Warning` and skips the constructor.
//! Missing id: the computed value is used.

use crate::config::CodegenScheme;

/// Rebuild the canonical line used for CRC, mirroring the Python:
/// `cleanline` construction before `binascii.crc32(a2b_qp(cleanline))`.
pub fn clean_line(declaration: &str, scheme: &CodegenScheme) -> String {
    // Python: removes `name:flags.N?true` optional-true params from the
    // line used for CRC, rewrites `<>` to spaces, applies synonyms
    // (`bytes` -> `string`), strips `{}` template braces.
    let mut line = declaration.to_string();
    for (from, to) in &scheme.synonyms {
        line = line.replace(from.as_str(), to.as_str());
    }
    line = line.replace(['<', '>'], " ");
    line = line.replace(['{', '}'], "");
    line
}

/// Compute the TL type id: CRC-32 of the cleaned declaration.
///
/// Matches Python `binascii.crc32(a2b_qp(cleanline))`. Note `a2b_qp` is a
/// quoted-printable decode — for the ASCII-only `.tl` content it is the
/// identity, so plain bytes CRC-32 is exact.
pub fn type_id(cleaned: &str) -> u32 {
    crc32fast::hash(cleaned.as_bytes())
}

/// Format like the `.tl` hex ids: lowercase hex without leading zeros
/// stripped for comparison (Python strips leading `0`s from the provided
/// id before comparing).
pub fn format_id(id: u32) -> String {
    format!("{id:08x}")
}

/// Compare a provided `#hexid` against the computed one, honoring
/// `typeIdExceptions`. Returns the id to use, or `None` if the constructor
/// must be skipped (mismatch warning, like Python's `continue`).
pub fn verify_or_compute(
    declaration: &str,
    provided_hex: Option<&str>,
    scheme: &CodegenScheme,
    warnings: &mut Vec<String>,
) -> Option<u32> {
    let cleaned = clean_line(declaration, scheme);
    let computed = type_id(&cleaned);
    match provided_hex {
        None => Some(computed),
        Some(hex) => {
            let stripped = hex.trim_start_matches('0');
            let expected = format_id(computed);
            let expected_stripped = expected.trim_start_matches('0');
            if stripped.eq_ignore_ascii_case(expected_stripped) {
                Some(computed)
            } else if scheme.type_id_exceptions.contains(declaration.trim()) {
                u32::from_str_radix(hex, 16).ok()
            } else {
                warnings.push(format!(
                    "Warning: type id mismatch for {declaration}: provided #{hex}, computed #{expected}"
                ));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_id_matches_known_value() {
        // `vector#1cb5c415 {t:Type} # [ t ] = Vector t;` — the one id
        // every TL implementation knows by heart.
        let scheme = CodegenScheme {
            synonyms: [("bytes".to_string(), "string".to_string())]
                .into_iter()
                .collect(),
            ..serde_json::from_str("{}").unwrap()
        };
        let cleaned = clean_line("vector {t:Type} # [ t ] = Vector t", &scheme);
        assert_eq!(format_id(type_id(&cleaned)), "1cb5c415");
    }
}
