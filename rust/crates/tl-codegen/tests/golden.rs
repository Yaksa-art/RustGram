//! Golden tests that run in CI with no network and no submodules, plus one
//! ignored e2e against the real Telegram scheme.
//!
//! Real-scheme setup (only for the `#[ignore]`d test):
//! 1. Check out Telegram Desktop so `Telegram/SourceFiles/mtproto/scheme`
//!    holds `api.tl` + `mtproto.tl`.
//! 2. `TL_SCHEME_DIR=/path/to/Telegram/SourceFiles/mtproto/scheme`.
//! 3. `cargo test -p tl-codegen --test golden -- --ignored`.
//! CI never sets `TL_SCHEME_DIR`, so that test stays skipped there.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tl_codegen::{read_and_generate, CodegenScheme};

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Minimal tdesktop-shaped config: no sections (non-read-write), no builtins,
/// single-data optimization off. Enough to drive the always-on `.h/.cpp` core.
fn minimal_config() -> CodegenScheme {
    serde_json::from_value(serde_json::json!({
        "namespaces": { "global": "", "creator": "" },
        "prefixes": { "type": "MTP", "data": "MTPD", "id": "mtpc", "construct": "MTP_" },
        "types": { "prime": "mtpPrime", "typeId": "mtpTypeId", "buffer": "mtpBuffer" },
        "sections": [],
        "builtin": [],
        "builtin_templates": [],
        "optimize_single_data": false,
    }))
    .expect("inline minimal config must deserialize")
}

/// Unique scratch dir per test: temp_dir + tag + pid + counter, so parallel
/// tests in one process never collide. Caller owns cleanup.
fn fresh_dir(tag: &str) -> PathBuf {
    let n = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "tl-codegen-golden-{tag}-{}-{n}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(path: &Path, content: &str) {
    fs::write(path, content).expect("write temp .tl file");
}

/// Two tiny inputs. No `#hexid`s on purpose: the parser computes the type id
/// via CRC when no id is present, so there is nothing to keep in sync.
fn write_synthetic_inputs(dir: &Path) -> (String, String) {
    let a = dir.join("a.tl");
    let b = dir.join("b.tl");
    write_file(
        &a,
        "// LAYER 1\nmyBoolTrue = Bool;\nmyBoolFalse = Bool;\nmyValue value:int = MyValue;\n",
    );
    write_file(&b, "---functions---\nmyGetValue = MyValue;\n");
    (
        a.to_string_lossy().into_owned(),
        b.to_string_lossy().into_owned(),
    )
}

#[test]
fn synthetic_end_to_end_is_idempotent() {
    let dir = fresh_dir("synthetic");
    let (a, b) = write_synthetic_inputs(&dir);
    let stem = dir.join("scheme").to_string_lossy().into_owned();
    let config = minimal_config();
    let inputs = [a.as_str(), b.as_str()];

    let first = read_and_generate(&inputs, &stem, &config).expect("generate synthetic scheme");

    assert!(
        first.header.contains("WARNING! All changes made"),
        "header must carry the warning banner"
    );
    assert!(
        first.header.contains("enum {"),
        "header must carry the type-id enum"
    );
    assert!(
        first.header.contains("class MTP"),
        "header must carry generated MTP classes"
    );
    assert!(
        first.source.contains("TypeCreator"),
        "source must carry the creator proxy"
    );

    let header_path = PathBuf::from(format!("{stem}.h"));
    let source_path = PathBuf::from(format!("{stem}.cpp"));
    let timestamp_path = PathBuf::from(format!("{stem}.timestamp"));
    assert_eq!(
        fs::read_to_string(&header_path).expect("read generated header"),
        first.header,
        "header on disk must match returned output"
    );
    assert_eq!(
        fs::read_to_string(&source_path).expect("read generated source"),
        first.source,
        "source on disk must match returned output"
    );
    assert_eq!(
        fs::read_to_string(&timestamp_path).expect("read timestamp"),
        "1",
        "timestamp file must contain the literal '1'"
    );

    // Idempotency: `write_if_changed` must skip both outputs on a second run.
    // (`.timestamp` is unconditionally rewritten by design, so only `.h`/`.cpp`
    // mtimes are compared here.)
    let header_mtime = fs::metadata(&header_path)
        .expect("stat header")
        .modified()
        .expect("header mtime");
    let source_mtime = fs::metadata(&source_path)
        .expect("stat source")
        .modified()
        .expect("source mtime");
    std::thread::sleep(Duration::from_millis(1100));
    let second = read_and_generate(&inputs, &stem, &config).expect("second generate run");
    assert_eq!(second.header, first.header, "header must be stable");
    assert_eq!(second.source, first.source, "source must be stable");
    assert_eq!(
        fs::metadata(&header_path)
            .expect("stat header")
            .modified()
            .expect("header mtime"),
        header_mtime,
        "header must not be rewritten when unchanged"
    );
    assert_eq!(
        fs::metadata(&source_path)
            .expect("stat source")
            .modified()
            .expect("source mtime"),
        source_mtime,
        "source must not be rewritten when unchanged"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn banner_prefix_is_stable() {
    let dir = fresh_dir("banner");
    let single = dir.join("single.tl");
    write_file(&single, "soloOne = Solo;\n");
    let path = single.to_string_lossy().into_owned();
    let stem = dir.join("scheme").to_string_lossy().into_owned();

    let outputs = read_and_generate(&[path.as_str()], &stem, &minimal_config())
        .expect("generate banner scheme");
    assert!(
        outputs.header.starts_with(
            "// WARNING! All changes made in this file will be lost!\n// Created from"
        ),
        "header must start with the exact warning banner"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Full-scheme parity against the real Telegram `.tl` files. Needs
/// `TL_SCHEME_DIR` pointing at `Telegram/SourceFiles/mtproto/scheme`
/// (holding `api.tl` + `mtproto.tl`); run with
/// `cargo test -p tl-codegen --test golden -- --ignored`.
#[test]
#[ignore = "needs TL_SCHEME_DIR (Telegram/SourceFiles/mtproto/scheme)"]
fn real_scheme_end_to_end() {
    let scheme_dir =
        std::env::var("TL_SCHEME_DIR").expect("TL_SCHEME_DIR must point at mtproto/scheme");
    let api = Path::new(&scheme_dir)
        .join("api.tl")
        .to_string_lossy()
        .into_owned();
    let mtproto = Path::new(&scheme_dir)
        .join("mtproto.tl")
        .to_string_lossy()
        .into_owned();
    assert!(
        Path::new(&api).exists(),
        "api.tl must exist in TL_SCHEME_DIR"
    );
    assert!(
        Path::new(&mtproto).exists(),
        "mtproto.tl must exist in TL_SCHEME_DIR"
    );

    // tdesktop-equivalent config: the exact JSON the real generator uses.
    let json = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/codegen_scheme.json"))
        .expect("read codegen_scheme.json");
    let config: CodegenScheme = serde_json::from_str(&json).expect("parse codegen_scheme.json");

    let dir = fresh_dir("real");
    let stem = dir.join("scheme").to_string_lossy().into_owned();
    let outputs = read_and_generate(&[api.as_str(), mtproto.as_str()], &stem, &config)
        .expect("generate real scheme");

    assert!(
        outputs.header.contains("kCurrentLayer = mtpPrime(229)"),
        "header must pin the current layer"
    );
    assert!(
        outputs.header.contains("MTPDmessage"),
        "header must contain the message data class"
    );

    let _ = fs::remove_dir_all(&dir);
}
