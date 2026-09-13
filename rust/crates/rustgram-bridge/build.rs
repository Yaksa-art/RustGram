//! Captures `rustc --version` into the `RUSTGRAM_RUSTC_VERSION` env var,
//! so the build fingerprint reports the real compiler, not a guess.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTUP_TOOLCHAIN");
    let version = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|| "unknown rustc".to_owned());
    println!("cargo:rustc-env=RUSTGRAM_RUSTC_VERSION={version}");
}
