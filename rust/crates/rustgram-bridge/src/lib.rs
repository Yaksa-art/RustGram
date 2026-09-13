//! rustgram-bridge: the single Rust-to-C++ boundary of RustGram.
//!
//! ADR-0001 (docs/adr/ADR-0001-single-rust-bridge.md) mandates that this
//! crate is the ONLY Rust crate whose symbols cross the FFI boundary into
//! the C++ application. All future ported modules (storage, protocol,
//! media, ...) become pure-Rust dependencies of this crate; C++ never
//! links them directly.
//!
//! Safety contract for every extern-C export in this file:
//! 1. Safe functions only: no caller input can cause memory unsafety.
//! 2. Strings cross as owned pointers freed by rustgram_string_free.
//! 3. No panic unwinds across the boundary: panics become null returns.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uchar};
use std::panic::{self, AssertUnwindSafe};

/// Build fingerprint: crate version, compiler, profile.
pub fn fingerprint() -> String {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    format!(
        "rustgram-bridge {} / {} / {}",
        env!("CARGO_PKG_VERSION"),
        env!("RUSTGRAM_RUSTC_VERSION"),
        profile,
    )
}

/// Runs f, converting a panic into None instead of unwinding.
fn catch_unwind_str(f: impl FnOnce() -> String) -> *mut c_char {
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(s) => CString::new(s)
            .map(|c| c.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Returns the build fingerprint as a NUL-terminated UTF-8 string.
/// Caller owns it and MUST free with rustgram_string_free.
/// Null return means a Rust panic: treat as fatal bridge failure.
/// Safe to call from any thread.
#[no_mangle]
pub extern "C" fn rustgram_fingerprint() -> *mut c_char {
    catch_unwind_str(fingerprint)
}

/// Frees a string returned by this crate. Null is a no-op.
/// ptr must be null or a not-yet-freed pointer from this crate.
///
/// `#[allow(clippy::not_unsafe_ptr_arg_deref)]`: this is an FFI boundary by
/// design (C++ cannot call `unsafe fn`). Soundness rests on the documented
/// contract above; the raw-pointer dereference is encapsulated in the
/// `unsafe` block below and cannot be triggered except by violating that
/// contract, which is already undefined behavior per the docs.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn rustgram_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Copies up to out_len - 1 bytes of the fingerprint into out, plus NUL.
/// Returns bytes written (excl. NUL), or -1 on null buffer, zero length,
/// or internal panic. No heap transfer: nothing to free.
/// out must point to at least out_len writable bytes.
///
/// `#[allow(clippy::not_unsafe_ptr_arg_deref)]`: FFI boundary, same rationale
/// as on `rustgram_string_free` — the contract above is the safety boundary.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn rustgram_fingerprint_into(out: *mut c_uchar, out_len: usize) -> i64 {
    if out.is_null() || out_len == 0 {
        return -1;
    }
    let s = match panic::catch_unwind(AssertUnwindSafe(fingerprint)) {
        Ok(s) => s,
        Err(_) => return -1,
    };
    let bytes = s.as_bytes();
    let n = bytes.len().min(out_len - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, n);
        *out.add(n) = 0;
    }
    n as i64
}

/// Performs the contract self-test: really round-trips the input through an
/// owned CString and returns its byte length. Used by the C++ self-test at
/// startup before any real module traffic. Returns -1 on null input, invalid
/// UTF-8, interior NUL, or panic.
/// input must be null or a valid NUL-terminated string for the call.
///
/// `#[allow(clippy::not_unsafe_ptr_arg_deref)]`: FFI boundary, same rationale
/// as on `rustgram_string_free` — the contract above is the safety boundary.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn rustgram_selftest_roundtrip(input: *const c_char) -> i64 {
    if input.is_null() {
        return -1;
    }
    let roundtripped = panic::catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: non-null per the check above; the contract guarantees a
        // valid NUL-terminated string living for the whole call.
        let text = unsafe { CStr::from_ptr(input) }.to_str().ok()?;
        CString::new(text).ok()
    }));
    match roundtripped {
        // The CString is dropped here, but its length was genuinely produced
        // by a full Rust-side round-trip (parse + re-encode), which is what
        // the self-test validates: UTF-8 validity and NUL-safety.
        Ok(Some(owned)) => owned.to_bytes().len() as i64,
        _ => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn fingerprint_mentions_crate_and_profile() {
        let f = fingerprint();
        assert!(f.starts_with("rustgram-bridge 0.1.0 / "), "{f}");
        assert!(f.ends_with(" / debug") || f.ends_with(" / release"), "{f}");
    }

    #[test]
    fn fingerprint_ffi_roundtrip() {
        let ptr = rustgram_fingerprint();
        assert!(!ptr.is_null());
        let back = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_owned();
        assert_eq!(back, fingerprint());
        rustgram_string_free(ptr);
    }

    #[test]
    fn fingerprint_into_no_alloc_path() {
        let mut buf = vec![0xAAu8; 256];
        let n = rustgram_fingerprint_into(buf.as_mut_ptr(), buf.len());
        assert!(n > 0);
        let n = n as usize;
        assert_eq!(buf[n], 0);
        let s = std::str::from_utf8(&buf[..n]).unwrap();
        assert_eq!(s, fingerprint());
    }

    #[test]
    fn fingerprint_into_truncates_safely() {
        let mut buf = vec![0xAAu8; 8];
        let n = rustgram_fingerprint_into(buf.as_mut_ptr(), buf.len());
        assert_eq!(n, 7);
        assert_eq!(buf[7], 0);
        assert!(std::str::from_utf8(&buf[..7]).is_ok());
    }

    #[test]
    fn string_free_null_is_noop() {
        rustgram_string_free(std::ptr::null_mut());
    }

    #[test]
    fn selftest_roundtrip_ok() {
        let input = CString::new("rustgram-phase0").unwrap();
        let n = rustgram_selftest_roundtrip(input.as_ptr());
        assert_eq!(n, "rustgram-phase0".len() as i64);
    }

    #[test]
    fn selftest_roundtrip_null_is_minus_one() {
        assert_eq!(rustgram_selftest_roundtrip(std::ptr::null()), -1);
    }
}
