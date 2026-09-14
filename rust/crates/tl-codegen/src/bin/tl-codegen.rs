//! `tl-codegen` CLI: mirrors `generate(scheme)` argument parsing
//! (`-o<stem>` / `-o <stem>`, then input files).

use std::fs;
use tl_codegen::{read_and_generate, CodegenScheme};

fn usage() -> ! {
    eprintln!("usage: tl-codegen --config <codegen_scheme.json> -o<stem> <file.tl>...");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut config_path: Option<String> = None;
    let mut output_stem: Option<String> = None;
    let mut inputs: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--config" {
            i += 1;
            config_path = args.get(i).cloned();
            if config_path.is_none() {
                usage();
            }
        } else if let Some(stem) = a.strip_prefix("-o") {
            if stem.is_empty() {
                i += 1;
                output_stem = args.get(i).cloned();
            } else {
                output_stem = Some(stem.to_string());
            }
            if output_stem.is_none() {
                usage();
            }
        } else if a == "-h" || a == "--help" {
            usage();
        } else {
            inputs.push(a.clone());
        }
        i += 1;
    }
    let (Some(config_path), Some(stem)) = (config_path, output_stem) else {
        usage()
    };
    if inputs.is_empty() {
        eprintln!("tl-codegen: input file required.");
        std::process::exit(2);
    }
    let config_text = fs::read_to_string(&config_path).unwrap_or_else(|e| {
        eprintln!("tl-codegen: cannot read {config_path}: {e}");
        std::process::exit(1);
    });
    let config: CodegenScheme = serde_json::from_str(&config_text).unwrap_or_else(|e| {
        eprintln!("tl-codegen: bad config {config_path}: {e}");
        std::process::exit(1);
    });
    if config.types.prime.is_empty() || config.types.buffer.is_empty() {
        if config.read_write_section() || config.write_serialization() {
            eprintln!("tl-codegen: required types not provided.");
            std::process::exit(1);
        }
    }
    let refs: Vec<&str> = inputs.iter().map(String::as_str).collect();
    if let Err(e) = read_and_generate(&refs, &stem, &config) {
        eprintln!("tl-codegen: {e}");
        std::process::exit(1);
    }
}
