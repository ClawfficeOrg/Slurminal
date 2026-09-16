//! GPUI migration delta checker.
//!
//! Parses a JSON [`MigrationDelta`][crate::gpui_migration::MigrationDelta] and returns
//! structured [`MigrationAction`][crate::gpui_migration::MigrationAction] items describing
//! API removals, renames, and new required imports between two GPUI versions.
//!
//! # Usage
//!
//! ```rust,no_run
//! use zoid_core::gpui_migration_checker::{check_delta, parse_delta_json};
//!
//! let json = r#"{
//!   "from_version": "0.2.0",
//!   "to_version": "0.3.0",
//!   "changes": [
//!     {
//!       "kind": "remove_method",
//!       "symbol": "Window::activate",
//!       "module": "gpui",
//!       "suggestion": "Use cx.activate_window() instead"
//!     }
//!   ]
//! }"#;
//!
//! let delta = parse_delta_json(json).unwrap();
//! let actions = check_delta(&delta);
//! for action in &actions {
//!     println!("{}", action.suggestion);
//! }
//! ```
//!
//! # Conservative guidance policy
//!
//! The checker surfaces all changes from the delta as actions — it does not attempt
//! automatic code edits.  Rename suggestions are advisory only; the user must verify
//! that each rename applies to their codebase.  See `docs/gpui_version_checker.md`
//! for the full guidance policy.

use std::fmt::{self, Display};

use crate::gpui_migration::{DeltaChange, MigrationAction, MigrationActionKind, MigrationDelta};

// ── Public API ────────────────────────────────────────────────────────────────

/// Parses a GPUI migration delta from a JSON string.
///
/// The expected format is documented in [`crate::gpui_migration`].
///
/// # Errors
///
/// Returns [`MigrationCheckerError`] when the input is not valid JSON or is
/// missing required fields.
pub fn parse_delta_json(json: &str) -> Result<MigrationDelta, MigrationCheckerError> {
    serde_json::from_str(json).map_err(|e| MigrationCheckerError::parse(e.to_string()))
}

/// Converts all changes in a [`MigrationDelta`] into [`MigrationAction`] items.
///
/// Every change entry in the delta is converted to an action regardless of severity.
/// Callers that want to filter to only breaking changes should call
/// [`is_breaking_delta`] first.
///
/// Returns an empty `Vec` when the delta has no changes.
#[must_use]
pub fn check_delta(delta: &MigrationDelta) -> Vec<MigrationAction> {
    delta.changes.iter().map(change_to_action).collect()
}

/// Returns `true` when the delta contains at least one change (i.e., is breaking).
///
/// An empty `changes` array indicates a no-op / non-breaking delta (e.g. a
/// patch-only release with no API surface changes).
#[must_use]
pub fn is_breaking_delta(delta: &MigrationDelta) -> bool {
    !delta.changes.is_empty()
}

/// Serialises a slice of [`MigrationAction`] values as a pretty-printed JSON object.
///
/// The output object contains `from_version`, `to_version`, and `actions` keys,
/// suitable for machine consumption in CI pipelines.
///
/// # Errors
///
/// Returns [`MigrationCheckerError`] when JSON serialisation fails (should not
/// occur with well-formed action values).
pub fn format_actions_as_json(
    from_version: &str,
    to_version: &str,
    actions: &[MigrationAction],
) -> Result<String, MigrationCheckerError> {
    #[derive(serde::Serialize)]
    struct Output<'a> {
        from_version: &'a str,
        to_version: &'a str,
        actions: &'a [MigrationAction],
    }

    serde_json::to_string_pretty(&Output {
        from_version,
        to_version,
        actions,
    })
    .map_err(|e| MigrationCheckerError::parse(e.to_string()))
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors produced by the migration checker.
#[derive(Debug)]
pub struct MigrationCheckerError {
    message: String,
}

impl MigrationCheckerError {
    fn parse(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the human-readable error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for MigrationCheckerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MigrationCheckerError {}

// ── Private helpers ───────────────────────────────────────────────────────────

fn change_to_action(change: &DeltaChange) -> MigrationAction {
    match change {
        DeltaChange::RemoveMethod {
            symbol,
            module,
            suggestion,
        } => MigrationAction {
            kind: MigrationActionKind::RemoveMethod {
                symbol: symbol.clone(),
                module: module.clone(),
            },
            suggestion: suggestion.clone(),
        },

        DeltaChange::RenameSymbol {
            old_symbol,
            new_symbol,
            module,
            suggestion,
        } => MigrationAction {
            kind: MigrationActionKind::RenameSymbol {
                old_symbol: old_symbol.clone(),
                new_symbol: new_symbol.clone(),
                module: module.clone(),
            },
            suggestion: suggestion.clone(),
        },

        DeltaChange::AddImport {
            import,
            module,
            suggestion,
        } => MigrationAction {
            kind: MigrationActionKind::AddImport {
                import: import.clone(),
                module: module.clone(),
            },
            suggestion: suggestion.clone(),
        },

        DeltaChange::Other {
            description,
            suggestion,
        } => MigrationAction {
            kind: MigrationActionKind::Other {
                description: description.clone(),
            },
            suggestion: suggestion.clone(),
        },
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gpui_migration::MigrationActionKind;

    const REMOVE_METHOD_JSON: &str = r#"{
        "from_version": "0.2.0",
        "to_version": "0.3.0",
        "changes": [
            {
                "kind": "remove_method",
                "symbol": "Window::activate",
                "module": "gpui",
                "suggestion": "Use cx.activate_window() instead"
            }
        ]
    }"#;

    const RENAME_SYMBOL_JSON: &str = r#"{
        "from_version": "0.2.0",
        "to_version": "0.3.0",
        "changes": [
            {
                "kind": "rename_symbol",
                "old_symbol": "ModelHandle",
                "new_symbol": "Model",
                "module": "gpui",
                "suggestion": "Replace ModelHandle<T> with Model<T>"
            }
        ]
    }"#;

    const ADD_IMPORT_JSON: &str = r#"{
        "from_version": "0.2.0",
        "to_version": "0.3.0",
        "changes": [
            {
                "kind": "add_import",
                "import": "use gpui::VisualContext;",
                "module": "gpui",
                "suggestion": "Add this import to access view rendering methods"
            }
        ]
    }"#;

    const EMPTY_DELTA_JSON: &str = r#"{
        "from_version": "0.2.0",
        "to_version": "0.2.1",
        "changes": []
    }"#;

    #[test]
    fn parse_remove_method_delta() {
        let delta = parse_delta_json(REMOVE_METHOD_JSON).expect("parse");
        assert_eq!(delta.from_version, "0.2.0");
        assert_eq!(delta.to_version, "0.3.0");
        assert_eq!(delta.changes.len(), 1);
    }

    #[test]
    fn parse_rename_symbol_delta() {
        let delta = parse_delta_json(RENAME_SYMBOL_JSON).expect("parse");
        assert_eq!(delta.changes.len(), 1);
        assert!(
            matches!(&delta.changes[0], DeltaChange::RenameSymbol { old_symbol, .. } if old_symbol == "ModelHandle")
        );
    }

    #[test]
    fn parse_add_import_delta() {
        let delta = parse_delta_json(ADD_IMPORT_JSON).expect("parse");
        assert_eq!(delta.changes.len(), 1);
        assert!(
            matches!(&delta.changes[0], DeltaChange::AddImport { import, .. } if import.contains("VisualContext"))
        );
    }

    #[test]
    fn parse_malformed_json_returns_error() {
        let result = parse_delta_json("{ not valid json !!!");
        assert!(result.is_err());
    }

    #[test]
    fn parse_unknown_kind_returns_error() {
        let json = r#"{"from_version":"0.2.0","to_version":"0.3.0","changes":[{"kind":"fly_to_moon","symbol":"X","module":"gpui","suggestion":""}]}"#;
        let result = parse_delta_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn check_delta_produces_remove_method_action() {
        let delta = parse_delta_json(REMOVE_METHOD_JSON).expect("parse");
        let actions = check_delta(&delta);
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0].kind, MigrationActionKind::RemoveMethod { symbol, .. } if symbol == "Window::activate")
        );
    }

    #[test]
    fn check_delta_produces_rename_symbol_action() {
        let delta = parse_delta_json(RENAME_SYMBOL_JSON).expect("parse");
        let actions = check_delta(&delta);
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0].kind, MigrationActionKind::RenameSymbol { old_symbol, new_symbol, .. }
                if old_symbol == "ModelHandle" && new_symbol == "Model")
        );
        assert!(actions[0].suggestion.contains("Model<T>"));
    }

    #[test]
    fn check_delta_produces_add_import_action() {
        let delta = parse_delta_json(ADD_IMPORT_JSON).expect("parse");
        let actions = check_delta(&delta);
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0].kind, MigrationActionKind::AddImport { import, .. } if import.contains("VisualContext"))
        );
    }

    #[test]
    fn empty_delta_produces_no_actions() {
        let delta = parse_delta_json(EMPTY_DELTA_JSON).expect("parse");
        let actions = check_delta(&delta);
        assert!(actions.is_empty());
    }

    #[test]
    fn is_breaking_delta_returns_true_when_changes_present() {
        let delta = parse_delta_json(REMOVE_METHOD_JSON).expect("parse");
        assert!(is_breaking_delta(&delta));
    }

    #[test]
    fn is_breaking_delta_returns_false_for_empty_changes() {
        let delta = parse_delta_json(EMPTY_DELTA_JSON).expect("parse");
        assert!(!is_breaking_delta(&delta));
    }

    #[test]
    fn format_actions_as_json_roundtrip() {
        let delta = parse_delta_json(REMOVE_METHOD_JSON).expect("parse");
        let actions = check_delta(&delta);
        let json = format_actions_as_json(&delta.from_version, &delta.to_version, &actions)
            .expect("format");

        // The JSON output must contain structural markers parseable by CI consumers.
        assert!(json.contains("\"from_version\""));
        assert!(json.contains("\"to_version\""));
        assert!(json.contains("\"actions\""));
        assert!(json.contains("remove_method"));
        assert!(json.contains("Window::activate"));

        // Verify the JSON is valid and round-trips through serde_json::Value.
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json output");
        assert_eq!(value["from_version"], "0.2.0");
        assert_eq!(value["to_version"], "0.3.0");
        assert!(value["actions"].is_array());
        assert_eq!(value["actions"].as_array().expect("array").len(), 1);
    }
}
