//! `CodegenScheme`: the Rust mirror of the dict passed to
//! `lib_tl/tl/generate_tl.py::generate()` by
//! `Telegram/SourceFiles/codegen/scheme/codegen_scheme.py`.
//!
//! Field-for-field port of what `readAndGenerate` reads out of `scheme`
//! (generate_tl.py lines 233-336). The Python side hands a dict; we read the
//! same shape from JSON (`codegen_scheme.json`), so the build graph never
//! needs Python once parity is proven.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Full codegen configuration. Missing keys default exactly like the
/// Python `.get(key, default)` calls do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodegenScheme {
    #[serde(default)]
    pub namespaces: Namespaces,
    #[serde(default)]
    pub prefixes: Prefixes,
    #[serde(default)]
    pub types: PrimitiveTypes,
    /// `sections`, e.g. `["read-write"]`. Only membership in
    /// `'read-write'` matters (`readWriteSection`).
    #[serde(default)]
    pub sections: Vec<String>,
    /// `flagInheritance`: key flag type must be a subset of value flag type.
    #[serde(default)]
    pub flag_inheritance: HashMap<String, String>,
    #[serde(default)]
    pub type_id_exceptions: HashSet<String>,
    #[serde(default)]
    pub renamed_types: HashMap<String, String>,
    /// Exact-line skips (`int ? = Int;`, both `vector` defs, ...).
    #[serde(default)]
    pub skip: HashSet<String>,
    #[serde(default)]
    pub builtin: HashSet<String>,
    #[serde(default)]
    pub builtin_templates: HashSet<String>,
    #[serde(default)]
    pub synonyms: HashMap<String, String>,
    #[serde(default)]
    pub builtin_include: String,
    #[serde(default)]
    pub nullable: HashSet<String>,
    #[serde(default)]
    pub optimize_single_data: bool,
    #[serde(default)]
    pub dump_to_text: Option<DumpToText>,
    #[serde(default)]
    pub conversion: Option<Conversion>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Namespaces {
    #[serde(default)]
    pub global: String,
    #[serde(default)]
    pub creator: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefixes {
    #[serde(rename = "type", default)]
    pub type_: String,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub construct: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrimitiveTypes {
    #[serde(rename = "prime", default)]
    pub prime: String,
    #[serde(rename = "typeId", default)]
    pub type_id: String,
    #[serde(rename = "buffer", default)]
    pub buffer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DumpToText {
    pub include: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversion {
    #[serde(default)]
    pub include: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub builtin_additional: Vec<String>,
    #[serde(default)]
    pub builtin_include_from: String,
    #[serde(default)]
    pub builtin_include_to: String,
}

impl CodegenScheme {
    /// `readWriteSection = 'read-write' in writeSections`.
    pub fn read_write_section(&self) -> bool {
        self.sections.iter().any(|s| s == "read-write")
    }

    /// `writeConversion = 'conversion' in scheme`.
    pub fn write_conversion(&self) -> bool {
        self.conversion.is_some()
    }

    /// `writeSerialization = 'dumpToText' in scheme`.
    pub fn write_serialization(&self) -> bool {
        self.dump_to_text.is_some()
    }

    /// `optimizeSingleData = 'optimizeSingleData' in scheme` — note: the
    /// Python checks key *presence*, but our JSON port uses an explicit bool.
    pub fn optimize_single_data(&self) -> bool {
        self.optimize_single_data
    }

    /// `idPrefix = prefixes.get('id') + '_'`.
    pub fn id_prefix(&self) -> String {
        format!("{}_", self.prefixes.id)
    }

    /// `creatorNamespaceFull`: global + creator joined like Python does.
    pub fn creator_namespace_full(&self) -> String {
        let g = self.namespaces.global.as_str();
        let c = self.namespaces.creator.as_str();
        if !c.is_empty() && !g.is_empty() {
            format!("{g}::{c}")
        } else {
            format!("{g}{c}")
        }
    }

    /// `isBuiltinType`: `builtin + builtinTemplates`.
    pub fn is_builtin_type(&self, name: &str) -> bool {
        self.builtin.contains(name) || self.builtin_templates.contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_python_get_fallbacks() {
        let s: CodegenScheme = serde_json::from_str("{}").unwrap();
        assert!(!s.read_write_section());
        assert!(!s.write_conversion());
        assert!(!s.write_serialization());
        assert_eq!(s.id_prefix(), "_");
        assert_eq!(s.creator_namespace_full(), "");
        assert!(!s.is_builtin_type("int"));
    }

    #[test]
    fn telegram_scheme_shape() {
        let json = serde_json::json!({
            "namespaces": { "creator": "MTP::details" },
            "prefixes": { "type": "MTP", "data": "MTPD", "id": "mtpc", "construct": "MTP_" },
            "types": { "prime": "mtpPrime", "typeId": "mtpTypeId", "buffer": "mtpBuffer" },
            "sections": ["read-write"],
            "optimize_single_data": true,
        });
        let s: CodegenScheme = serde_json::from_value(json).unwrap();
        assert!(s.read_write_section());
        assert_eq!(s.id_prefix(), "mtpc_");
        assert_eq!(s.creator_namespace_full(), "MTP::details");
    }
}
