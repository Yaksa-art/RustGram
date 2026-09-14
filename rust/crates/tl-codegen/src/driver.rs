//! Driver: `readAndGenerate` orchestration (M1-M2 fill the parse core;
//! M3-M6 fill the emitters). M0 wires CLI -> config -> inputs -> outputs.

use crate::config::CodegenScheme;
use crate::emit::{conversion, dump, header, source, write_if_changed};
use crate::tl::read_inputs;
use std::path::Path;

/// Outputs the generator produces for one `-o<stem>`.
#[derive(Debug, Default)]
pub struct Outputs {
    /// `scheme.h` / `scheme.cpp` (always).
    pub header: String,
    pub source: String,
    /// `-dump_to_text.h/.cpp` (only when `dump_to_text` configured).
    pub dump_header: Option<String>,
    pub dump_source: Option<String>,
    /// `-conversion-*.h/.cpp` (only when `conversion` configured).
    pub conversion_from_header: Option<String>,
    pub conversion_from_source: Option<String>,
    pub conversion_to_header: Option<String>,
    pub conversion_to_source: Option<String>,
}

/// Full pipeline: read `.tl` files, parse, emit, idempotent-write all
/// outputs plus `.timestamp` (Python lines 1635-1700 behavior).
pub fn read_and_generate(
    input_files: &[&str],
    output_stem: &str,
    config: &CodegenScheme,
) -> std::io::Result<Outputs> {
    let inputs = read_inputs(input_files)?;
    let parsed = crate::parse_scheme(&inputs, config);

    let outputs = Outputs {
        header: header::emit(&parsed, config),
        source: source::emit(&parsed, config),
        dump_header: config
            .write_serialization()
            .then(|| dump::emit_header(&parsed, config)),
        dump_source: config
            .write_serialization()
            .then(|| dump::emit_source(&parsed, config)),
        conversion_from_header: config
            .write_conversion()
            .then(|| conversion::emit_from_header(&parsed, config)),
        conversion_from_source: config
            .write_conversion()
            .then(|| conversion::emit_from_source(&parsed, config)),
        conversion_to_header: config
            .write_conversion()
            .then(|| conversion::emit_to_header(&parsed, config)),
        conversion_to_source: config
            .write_conversion()
            .then(|| conversion::emit_to_source(&parsed, config)),
    };

    write_if_changed(Path::new(&format!("{output_stem}.h")), &outputs.header)?;
    write_if_changed(Path::new(&format!("{output_stem}.cpp")), &outputs.source)?;
    if let Some(ref h) = outputs.dump_header {
        write_if_changed(Path::new(&format!("{output_stem}-dump_to_text.h")), h)?;
    }
    if let Some(ref s) = outputs.dump_source {
        write_if_changed(Path::new(&format!("{output_stem}-dump_to_text.cpp")), s)?;
    }
    if config.write_conversion() {
        for (suffix, content) in [
            ("-conversion-from.h", &outputs.conversion_from_header),
            ("-conversion-from.cpp", &outputs.conversion_from_source),
            ("-conversion-to.h", &outputs.conversion_to_header),
            ("-conversion-to.cpp", &outputs.conversion_to_source),
        ] {
            if let Some(text) = content {
                write_if_changed(Path::new(&format!("{output_stem}{suffix}")), text)?;
            }
        }
    }
    // `.timestamp`: always rewritten (touches the CMake custom-command
    // output so downstream rebuilds trigger exactly like Python).
    std::fs::write(
        Path::new(&format!("{output_stem}.timestamp")),
        format!("layer {}\n", parsed.layer),
    )?;
    let _ = inputs.layer;
    Ok(outputs)
}
