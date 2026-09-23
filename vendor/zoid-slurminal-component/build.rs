//! Build script for zoid-slurminal-component.
//!
//! The heavy lifting (Zig build of libghostty-vt) is handled by the
//! `libghostty-vt-sys` crate's own build.rs. This script validates the
//! toolchain prerequisites and provides clear error messages.

fn main() {
    // Skip validation on docs.rs (no Zig) and Miri (no native libs).
    if std::env::var("DOCS_RS").is_ok() || std::env::var("CARGO_CFG_MIRI").is_ok() {
        return;
    }

    // Check that Zig is available — libghostty-vt-sys needs it to build
    // the vendored Ghostty VT core.
    let zig_available = std::process::Command::new("zig")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !zig_available {
        let zig_path = std::env::var("ZIG").unwrap_or_default();
        let zig_available_at_path = if !zig_path.is_empty() {
            std::process::Command::new(&zig_path)
                .arg("version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        } else {
            false
        };

        if !zig_available_at_path {
            println!(
                "cargo:warning=\
                 Zig toolchain not found. libghostty-vt requires Zig to build. \
                 Install Zig 0.14.1+ from https://ziglang.org/download/ and ensure \
                 it is in PATH, or set the ZIG environment variable to the zig binary path."
            );
        }
    }

    // Re-export the include path from libghostty-vt-sys so downstream can
    // find the ghostty/vt.h headers if needed.
    if let Ok(include) = std::env::var("DEP_GHOSTTY_VT_INCLUDE") {
        println!("cargo:include={include}");
    }
}
