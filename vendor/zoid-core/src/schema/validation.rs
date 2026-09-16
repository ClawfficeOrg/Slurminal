//! Schema validation error types for `zoid-project.json`.

use std::fmt::{self, Display};

/// A validation failure produced while checking a `zoid-project.json` document.
#[derive(Debug)]
pub struct SchemaValidationError {
    kind: SchemaValidationErrorKind,
    message: String,
}

impl SchemaValidationError {
    /// Creates an "empty required field" error.
    pub(crate) fn empty_field(field: &str) -> Self {
        Self {
            kind: SchemaValidationErrorKind::EmptyField,
            message: format!("field `{field}` must not be empty"),
        }
    }

    /// Creates an "invalid field value" error with a reason string.
    pub(crate) fn invalid_field(field: &str, reason: &str) -> Self {
        Self {
            kind: SchemaValidationErrorKind::InvalidField,
            message: format!("field `{field}` is invalid: {reason}"),
        }
    }

    /// Creates a path-traversal error.
    pub(crate) fn path_traversal(field: &str) -> Self {
        Self {
            kind: SchemaValidationErrorKind::PathTraversal,
            message: format!("field `{field}` must not contain `..` path components"),
        }
    }

    /// Creates an unsafe-path error.
    pub(crate) fn unsafe_path(field: &str) -> Self {
        Self {
            kind: SchemaValidationErrorKind::UnsafePath,
            message: format!("field `{field}` specifies an unsafe filesystem root"),
        }
    }

    /// Returns the machine-readable error kind.
    #[must_use]
    pub fn kind(&self) -> SchemaValidationErrorKind {
        self.kind
    }
}

impl Display for SchemaValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SchemaValidationError {}

/// Machine-readable categories for [`SchemaValidationError`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaValidationErrorKind {
    /// A required field was empty or whitespace-only.
    EmptyField,
    /// A field value did not match the required format.
    InvalidField,
    /// A path field contained `..` components (path-traversal attempt).
    PathTraversal,
    /// A path field pointed at an unsafe filesystem location.
    UnsafePath,
}

/// Convenience alias for results produced by schema validation.
pub type SchemaValidationResult<T> = Result<T, SchemaValidationError>;
