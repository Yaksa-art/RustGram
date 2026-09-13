//! rust-loc: the RustGram migration metric.
//!
//! Walks the repository and counts lines of code, split into three buckets:
//! Rust (ours, growing), C++ family (theirs, shrinking), and everything else
//! (build files, resources, docs). Output is one human-readable block plus,
//! with --json, a machine-readable object for CI trend tracking.
//!
//! Usage: cargo run -p xtask-rust-loc [-- --json] [--root <path>]
//!
//! The metric this feeds is defined in ROADMAP.md ("Metrics"): the Rust LoC
//! share must grow monotonically; the absolute C++ count documents progress.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const RUST_EXTS: &[&str] = &["rs"];
const CPP_EXTS: &[&str] = &["cpp", "h", "hpp", "c", "cc", "cxx", "mm", "m"];

const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "out",
    "build",
    "ThirdParty",
    "Libraries",
    "node_modules",
    ".venv",
    "__pycache__",
];

const SKIP_FILES: &[&str] = &["Cargo.lock"];

#[derive(Default)]
struct Counts {
    rust_files: u64,
    rust_lines: u64,
    cpp_files: u64,
    cpp_lines: u64,
    other_files: u64,
    other_lines: u64,
}

fn count_lines(path: &Path) -> u64 {
    fs::read(path)
        .map(|bytes| {
            let text = String::from_utf8_lossy(&bytes);
            text.lines().count() as u64
        })
        .unwrap_or(0)
}

fn walk(dir: &Path, counts: &mut Counts) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(&path, counts);
            }
            continue;
        }
        if SKIP_FILES.contains(&name.as_str()) {
            continue;
        }
        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .unwrap_or_default();
        let lines = count_lines(&path);
        if RUST_EXTS.iter().any(|e| *e == ext) {
            counts.rust_files += 1;
            counts.rust_lines += lines;
        } else if CPP_EXTS.iter().any(|e| *e == ext) {
            counts.cpp_files += 1;
            counts.cpp_lines += lines;
        } else {
            counts.other_files += 1;
            counts.other_lines += lines;
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let as_json = args.iter().any(|a| a == "--json");
    let root = args
        .windows(2)
        .find(|w| w[0] == "--root")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| PathBuf::from("."));

    let mut counts = Counts::default();
    walk(&root, &mut counts);

    let total_code = counts.rust_lines + counts.cpp_lines;
    let share = if total_code > 0 {
        100.0 * counts.rust_lines as f64 / total_code as f64
    } else {
        0.0
    };

    if as_json {
        println!(
            concat!(
                "{{\"rust_files\":{},\"rust_lines\":{},",
                "\"cpp_files\":{},\"cpp_lines\":{},",
                "\"other_files\":{},\"other_lines\":{},",
                "\"rust_share_pct\":{:.4}}}",
            ),
            counts.rust_files,
            counts.rust_lines,
            counts.cpp_files,
            counts.cpp_lines,
            counts.other_files,
            counts.other_lines,
            share,
        );
    } else {
        println!("RustGram LoC metric (--json for machine output)");
        println!(
            "  Rust:  {:>8} files  {:>10} lines",
            counts.rust_files, counts.rust_lines
        );
        println!(
            "  C++:   {:>8} files  {:>10} lines",
            counts.cpp_files, counts.cpp_lines
        );
        println!(
            "  Other: {:>8} files  {:>10} lines",
            counts.other_files, counts.other_lines
        );
        println!("  Rust share of code: {share:.2}%");
    }
}
