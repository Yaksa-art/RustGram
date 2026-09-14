//! Line-level input handling: port of `readInputs` (lines 9-23) and the
//! comment/tag predicates (lines 184-217).
//!
//! `readInputs` reads the `.tl` files, extracts `LAYER` (last wins), and
//! prepends the `---types---` sentinel. The predicates filter bots-only,
//! mobile-only, and nullable annotations from `@description`-style comments.

use std::fs;
use std::path::Path;

/// Result of reading the input `.tl` files.
pub struct TlInputs {
    /// All lines with the `---types---` sentinel prepended.
    pub lines: Vec<String>,
    /// Last `// LAYER <n>` seen (229 for the current scheme).
    pub layer: u32,
    /// Basenames of the input files, in order (`names` in Python).
    pub names: Vec<String>,
}

/// Read `.tl` files: extract `LAYER`, keep every other line verbatim,
/// prepend `---types---`. Mirrors `readInputs` exactly.
pub fn read_inputs(files: &[&str]) -> std::io::Result<TlInputs> {
    let mut all_lines = vec!["---types---".to_string()];
    let mut layer = 0u32;
    let mut names = Vec::with_capacity(files.len());
    for file in files {
        let path = Path::new(file);
        let base = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.to_string());
        names.push(base);
        let text = fs::read_to_string(path)?;
        for raw in text.lines() {
            if let Some(n) = parse_layer(raw) {
                layer = n;
            } else {
                all_lines.push(raw.to_string());
            }
        }
    }
    Ok(TlInputs {
        lines: all_lines,
        layer,
        names,
    })
}

/// `// LAYER (\d+)` — last occurrence wins.
fn parse_layer(line: &str) -> Option<u32> {
    let line = line.trim();
    let rest = line.strip_prefix("//")?.trim();
    let num = rest.strip_prefix("LAYER")?.trim();
    num.parse::<u32>().ok()
}

/// `endsWithForTag(comments, tag, ending)`: looks for `@<tag> ...` in the
/// accumulated comments and checks the text up to the next `@` ends with
/// `; <ending>` (with the same follow-char tolerance as Python).
pub fn ends_with_for_tag(comments: &str, tag: &str, ending: &str) -> bool {
    let needle = format!("@{tag} ");
    let pos = match comments.find(&needle) {
        Some(p) => p,
        None => return false,
    };
    let tail = &comments[pos + needle.len()..];
    let line = match tail.find('@') {
        Some(i) => &tail[..i],
        None => tail,
    };
    let stripped = line.trim();
    let fullending = format!("; {}", ending.trim());
    if stripped.len() < fullending.len() {
        return false;
    }
    if stripped.ends_with(&fullending) {
        return true;
    }
    for sep in [
        ".",
        ";",
        " if",
        " to",
        " otherwise",
        " unless",
        " even",
        " for",
    ] {
        if stripped.contains(&format!("{fullending}{sep}")) {
            return true;
        }
    }
    // Python prints a WARNING here when `ending` merely appears; we mirror
    // that as a debug print so golden runs can compare warning streams.
    if line.contains(ending) {
        eprintln!("WARNING: Found \"{ending}\" in \"{stripped}\"");
    }
    false
}

/// `paramNameTag`: `description` -> `param_description`, else identity.
pub fn param_name_tag(name: &str) -> &str {
    if name == "description" {
        "param_description"
    } else {
        name
    }
}

pub fn is_bots_only_line(comments: &str) -> bool {
    ends_with_for_tag(comments, "description", "for bots only")
        || ends_with_for_tag(comments, "description", "bots only")
}

pub fn is_bots_only_param(comments: &str, name: &str) -> bool {
    ends_with_for_tag(comments, param_name_tag(name), "for bots only")
        || ends_with_for_tag(comments, param_name_tag(name), "bots only")
}

pub fn is_mobile_only_line(comments: &str) -> bool {
    ends_with_for_tag(
        comments,
        "description",
        "for official mobile applications only",
    )
}

pub fn is_nullable_vector(comments: &str, name: &str) -> bool {
    name.ends_with('s')
        && ends_with_for_tag(
            comments,
            param_name_tag(name),
            &format!("{name} may be null"),
        )
}

pub fn is_nullable_param(comments: &str, name: &str) -> bool {
    ends_with_for_tag(comments, param_name_tag(name), "may be null")
        || ends_with_for_tag(comments, param_name_tag(name), "pass null")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_last_wins_and_sentinel_first() {
        let dir = std::env::temp_dir();
        let a = dir.join("tl_test_a.tl");
        let b = dir.join("tl_test_b.tl");
        std::fs::write(&a, "boolTrue#997275b5 = Bool;\n// LAYER 100\n").unwrap();
        std::fs::write(&b, "boolFalse#bc799737 = Bool;\n// LAYER 229\n").unwrap();
        let inputs = read_inputs(&[&a.to_string_lossy(), &b.to_string_lossy()]).unwrap();
        assert_eq!(inputs.names, vec!["tl_test_a.tl", "tl_test_b.tl"]);
        assert_eq!(inputs.layer, 229);
        assert_eq!(inputs.lines[0], "---types---");
        assert!(inputs.lines.iter().any(|l| l.contains("boolTrue")));
        assert!(inputs.lines.iter().any(|l| l.contains("boolFalse")));
        assert!(!inputs.lines.iter().any(|l| l.contains("LAYER")));
    }

    #[test]
    fn bots_only_tag() {
        assert!(is_bots_only_line(
            "@description something for bots only; tail"
        ));
        assert!(!is_bots_only_line("@description just a normal thing"));
    }
}
