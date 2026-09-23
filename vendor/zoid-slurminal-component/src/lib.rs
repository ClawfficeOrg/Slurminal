//! Terminal component for Zoid: `libghostty-vt` bindings plus a GPUI surface.
//!
//! This crate is the reusable half of Slurminal. It owns two separable concerns
//! that start life together and may split later:
//!
//! - **Bindings** — safe Rust wrappers over `libghostty-vt`, the cross-platform
//!   C/Zig terminal core from Ghostty. Parsing, terminal state, grid contents.
//! - **Surface** — a GPUI element that renders terminal state and routes input,
//!   sizing, selection, and scrollback.
//!
//! Nothing here knows about Paseo. Slurminal and Specttyr both consume it.
//!
//! # Status
//!
//! Bindings (9.1), surface (9.2), retained [`surface::TerminalView`] (9.3.1)
//! and PTY driver ([`pty`]) are in place. Slurminal drives a live shell
//! through them end to end (9.3.5): output via `TerminalView::feed`, input
//! and query responses via `TerminalEvent::Input`.

pub mod pty;
pub mod surface;
pub mod terminal;

/// Returns the GPUI crate version this component is built against.
///
/// Mirrors [`zoid_gpui::targeted_gpui_version`] so generated projects and the
/// component agree on what they were verified against.
#[must_use]
pub const fn targeted_gpui_version() -> Option<&'static str> {
    Some("0.3")
}

/// Identifies the `libghostty-vt` revision this crate's bindings target.
///
/// Pins the vendored `zoid-ghostty-vt` crate version (0.3.0, new C API era:
/// `Terminal::new(cols, rows)`, scrollback via setters) and the upstream
/// Ghostty pin (`22d13172`). The safe Rust bindings are vendored from
/// ClawfficeOrg/libghostty-rs; the C API comes from ghostty-org/ghostty.
#[must_use]
pub const fn targeted_libghostty_rev() -> Option<&'static str> {
    Some("zoid-ghostty-vt 0.3.0 / ghostty 22d13172")
}
