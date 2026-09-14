//! Intermediate representation: typed structs replacing the Python
//! 13-tuples and parallel dicts (`typesDict`, `funcsDict`, ...).
//!
//! Python `entry = [name, typeid, prmsList, prms, hasFlags, hasFlags64,
//! conditionsList, conditions, trivialConditions, isTemplate,
//! nullablePrms, nullableVectors, botsOnlyPrms]` maps onto `Constructor`
//! below, field for field.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Whole parsed scheme: ordered type/function tables + layer + warnings.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Scheme {
    /// Last `// LAYER` seen (229 for the current scheme).
    pub layer: u32,
    /// Basenames of the input `.tl` files, in order (`names` in Python,
    /// used for the `Created from '...' by 'generate.py'` banner).
    #[serde(default)]
    pub input_names: Vec<String>,
    /// `typesDict` + `typesList`: result type -> definition, in order.
    pub types: IndexMap<String, TypeDef>,
    /// `funcsDict` + `funcsList`: same for `---functions---`.
    pub funcs: IndexMap<String, TypeDef>,
    /// `Warning ...` lines Python prints on CRC mismatch (collected, not
    /// printed, so tests can compare warning streams).
    pub warnings: Vec<String>,
    /// `enums[]`: `\tmtpc<name> = 0x<id>` lines in emission order.
    pub enums: Vec<String>,
}

/// One result type (`restype`): all its constructors plus derived flags.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    /// Normalized bare result-type name, e.g. `res_p_q`.
    pub restype: String,
    /// Boxed alias from `TypesDict`.
    pub boxed_name: String,
    pub ctors: Vec<Constructor>,
    /// `withType = len(v) > 1` (polymorphic).
    pub with_type: bool,
    /// Has storable fields.
    pub with_data: bool,
    /// `restype in nullableTypes`.
    pub nullable: bool,
}

/// One constructor: `name#id params = Result;`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Constructor {
    /// `normalizedName(name)`: `ns.cons` -> `ns_cons`.
    pub name: String,
    /// Original TL name before `renamedTypes` (for `// RPC method` comments).
    #[serde(default)]
    pub tl_name: String,
    /// `Name` mangling (lines 409-414): `ns.cons` -> `ns_Cons`, `foo` -> `Foo`.
    #[serde(default)]
    pub alias_name: String,
    /// `normalizedBareName(name)`: `ns.Box` -> `ns_box`.
    pub bare_name: String,
    /// `fullTypeName` applied (`MTP` prefix).
    pub full_name: String,
    /// `fullDataName` applied (`MTPD` prefix).
    pub full_data_name: String,
    pub type_id: u32,
    /// Ordered params (`prmsList`).
    pub params: Vec<Param>,
    pub has_flags: bool,
    pub has_flags64: bool,
    /// `flags` field name (`hasFlags`), empty when none.
    #[serde(default)]
    pub flags_name: String,
    /// `flags2` field name (`hasFlags64`), empty when none.
    #[serde(default)]
    pub flags64_name: String,
    /// Template query param name (`isTemplate`), empty when none.
    #[serde(default)]
    pub template_param: String,
    /// `{X:Type}` placeholder var (`hasTemplate`), empty when none.
    #[serde(default)]
    pub template_var: String,
    /// `(pname, bit)` in order (`conditionsList`).
    pub conditions: Vec<(String, u32)>,
    /// `Type == true` flags (`trivialConditions`).
    pub trivial_conditions: HashSet<String>,
    pub is_template: bool,
    pub has_template: bool,
    pub nullable_params: HashSet<String>,
    pub nullable_vectors: HashSet<String>,
    pub bots_only_params: HashSet<String>,
}

/// One `pname:type` parameter after `handleTemplate` resolution.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    /// Normalized type after `handleTemplate` + `optionalInVector`.
    pub tl_type: String,
    pub raw_tl_type: String,
    /// Flag bit if conditional (`flags.N?`), `None` if unconditional.
    pub flag_bit: Option<u32>,
    pub is_true_flag: bool,
}
