//! Differential (golden) test: Rust output must be byte-identical to the
//! Python generator on the real scheme (M3-M4 fill the emitters; until
//! then these are `#[ignore]`d so the crate builds without fixtures).
//!
//! Set `TL_SCHEME_DIR` to `Telegram/SourceFiles/mtproto/scheme` to run:
//! 1. `python3 codegen_scheme.py -o $TMP/expected/scheme api.tl mtproto.tl`
//!    (same command as `generate_scheme.cmake`).
//! 2. `tl-codegen --config codegen_scheme.json -o $TMP/actual/scheme ...`
//! 3. Byte-diff the 4 files (8 when `conversion` configured).

#[test]
#[ignore = "needs TL_SCHEME_DIR + Python lib_tl; enabled at M3"]
fn golden_byte_identical() {
    let dir = std::env::var("TL_SCHEME_DIR").expect("TL_SCHEME_DIR not set");
    let _ = dir;
    // Implemented at M3 (scheme.h/.cpp), extended at M4 (dump_to_text).
}
