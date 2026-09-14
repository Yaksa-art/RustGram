//! Emitter scaffolding: `Writer` (indent + push) and idempotent writes.
//!
//! The Python emitter is 13 string accumulators concatenated in fixed order
//! (lines 1094-1635). We mirror that with one `Writer` per accumulator so
//! the port maps 1:1 and whitespace drift is visible immediately in golden
//! diffs. No templating engine — raw strings only (see plan M3 rationale).

pub mod conversion;
pub mod dump;
pub mod header;
pub mod source;

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// Indented C++ text accumulator.
#[derive(Debug, Default)]
pub struct Writer {
    buf: String,
    indent: usize,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn indent(&mut self) {
        self.indent += 1;
    }

    pub fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    /// Push one line at the current indent.
    pub fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.buf.push('\t');
        }
        self.buf.push_str(text);
        self.buf.push('\n');
    }

    /// Push raw text verbatim (for pre-formatted blocks).
    pub fn raw(&mut self, text: &str) {
        self.buf.push_str(text);
    }

    pub fn into_string(self) -> String {
        self.buf
    }
}

/// Write `path` only if the content changed (Python lines 1635-1700).
/// Returns `true` if the file was written.
pub fn write_if_changed(path: &Path, content: &str) -> std::io::Result<bool> {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == content {
            return Ok(false);
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(true)
}

/// Format a line the way Python string-concat does (kept explicit so the
/// `write!` import has one home).
#[allow(dead_code)]
pub fn push(w: &mut Writer, args: std::fmt::Arguments<'_>) {
    let mut s = String::new();
    let _ = s.write_fmt(args);
    w.raw(&s);
}
