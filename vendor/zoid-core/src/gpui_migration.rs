//! Types for GPUI migration deltas and concrete migration actions.
//!
//! A [`MigrationDelta`] is the input consumed by the migration checker: a JSON document
//! that lists API changes between two GPUI versions.  The checker converts each
//! [`DeltaChange`] into a [`MigrationAction`] that the CLI can display to the user.
//!
//! # Delta JSON format
//!
//! ```json
//! {
//!   "from_version": "0.2.0",
//!   "to_version": "0.3.0",
//!   "changes": [
//!     {
//!       "kind": "remove_method",
//!       "symbol": "Window::activate",
//!       "module": "gpui",
//!       "suggestion": "Use cx.activate_window() instead"
//!     },
//!     {
//!       "kind": "rename_symbol",
//!       "old_symbol": "ModelHandle",
//!       "new_symbol": "Model",
//!       "module": "gpui",
//!       "suggestion": "Replace ModelHandle<T> with Model<T>"
//!     }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};

// ── Input types (delta) ───────────────────────────────────────────────────────

/// A parsed GPUI semver migration delta describing breaking changes between two versions.
///
/// Load one with [`crate::gpui_migration_checker::parse_delta_json`].
#[derive(Debug, Clone, Deserialize)]
pub struct MigrationDelta {
    /// The GPUI version this delta applies from (e.g. `"0.2.0"`).
    pub from_version: String,
    /// The GPUI version this delta upgrades to (e.g. `"0.3.0"`).
    pub to_version: String,
    /// API changes included in this delta.
    pub changes: Vec<DeltaChange>,
}

/// A single API change in a migration delta.
///
/// Serialised with an internal `"kind"` tag so the JSON discriminant is
/// human-readable (e.g. `"kind": "remove_method"`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeltaChange {
    /// A method or function was removed entirely.
    RemoveMethod {
        /// Full symbol path that was removed (e.g. `"Window::activate"`).
        symbol: String,
        /// The GPUI module that contained the symbol (e.g. `"gpui"`).
        module: String,
        /// Guidance for migrating away from this symbol.
        suggestion: String,
    },

    /// A symbol was renamed.
    RenameSymbol {
        /// The old symbol name (e.g. `"ModelHandle"`).
        old_symbol: String,
        /// The new symbol name (e.g. `"Model"`).
        new_symbol: String,
        /// The GPUI module containing the renamed symbol.
        module: String,
        /// Additional migration guidance.
        suggestion: String,
    },

    /// A new import path is now required to use an API.
    AddImport {
        /// The full import path to add (e.g. `"use gpui::VisualContext;"`).
        import: String,
        /// The GPUI module that requires this import.
        module: String,
        /// Guidance on when this import is needed.
        suggestion: String,
    },

    /// A non-specific breaking or notable change.
    Other {
        /// Human-readable description of the change.
        description: String,
        /// Suggested user action.
        suggestion: String,
    },
}

// ── Output types (actions) ────────────────────────────────────────────────────

/// A concrete migration action surfaced by the checker for display to the user.
///
/// Produced by [`crate::gpui_migration_checker::check_delta`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MigrationAction {
    /// What kind of migration is required.
    #[serde(flatten)]
    pub kind: MigrationActionKind,
    /// Human-readable suggestion for how to handle this action.
    pub suggestion: String,
}

/// The specific kind of migration action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MigrationActionKind {
    /// A method or function was removed and must be replaced.
    RemoveMethod {
        /// The removed symbol path.
        symbol: String,
        /// The module that contained it.
        module: String,
    },

    /// A symbol was renamed and references must be updated.
    RenameSymbol {
        /// The old symbol name.
        old_symbol: String,
        /// The replacement symbol name.
        new_symbol: String,
        /// The module containing the renamed symbol.
        module: String,
    },

    /// A new import must be added to compile against the updated API.
    AddImport {
        /// The import path to add.
        import: String,
        /// The module requiring this import.
        module: String,
    },

    /// An unstructured breaking change with a free-form description.
    Other {
        /// Description of the change.
        description: String,
    },
}
