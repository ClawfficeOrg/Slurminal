//! Shared domain types and diagnostics for Zoid.

pub mod gpui_migration;
pub mod gpui_migration_checker;
pub mod gpui_version;
pub mod import;
pub mod migration;
pub mod schema;
pub mod validation;
pub mod zml;

/// The current Zoid package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns a short human-readable description for command-line output.
#[must_use]
pub fn about_line() -> String {
    format!("Zoid {VERSION} - GPUI app starter generator")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn about_line_includes_version() {
        let line = about_line();

        assert!(line.contains(VERSION));
        assert!(line.contains("GPUI"));
    }
}
