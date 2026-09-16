//! Error types for ZML parsing and validation.

use std::error::Error;
use std::fmt::{self, Display};

/// A ZML parse or validation failure.
#[derive(Debug)]
pub struct ZmlError {
    kind: ZmlErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl ZmlError {
    pub(crate) fn new(kind: ZmlErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    pub(crate) fn with_source(
        kind: ZmlErrorKind,
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Returns the machine-readable error kind.
    #[must_use]
    pub fn kind(&self) -> ZmlErrorKind {
        self.kind
    }
}

impl Display for ZmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ZmlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|s| s as &(dyn Error + 'static))
    }
}

/// Machine-readable ZML error categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZmlErrorKind {
    /// A prop or binding name was empty.
    EmptyName,
    /// A binding name contained invalid characters.
    InvalidBindingName,
    /// A theme override path was empty.
    ThemePathEmpty,
    /// A theme override path exceeded the maximum allowed depth.
    ThemePathTooDeep,
    /// An event handler had neither an `action_id` nor `handler_code`.
    MissingEventHandler,
    /// TOML deserialization failed.
    TomlParse,
    /// TOML serialization failed.
    TomlSerialize,
    /// JSON deserialization failed.
    JsonParse,
    /// JSON serialization failed.
    JsonSerialize,
    /// A `pattern_id` value was empty, over-long, or used an invalid charset.
    InvalidPatternId,
    /// A `pattern_id` referenced a pattern the document does not define.
    UnknownPatternId,
    /// Two patterns in a document shared an id.
    DuplicatePatternId,
    /// Pattern references formed a cycle, which would expand without bound.
    PatternCycle,
    /// A node tree exceeded the maximum depth explored during pattern
    /// expansion.
    PatternTooDeep,
    /// An overlay `id` was empty, over-long, or not a valid Rust identifier.
    InvalidOverlayId,
    /// Two overlays in a document shared an id.
    DuplicateOverlayId,
    /// An overlay `kind` was not `Modal` or `Drawer`.
    UnknownOverlayKind,
}

impl From<toml::de::Error> for ZmlError {
    fn from(e: toml::de::Error) -> Self {
        Self::with_source(ZmlErrorKind::TomlParse, e.to_string(), e)
    }
}

impl From<toml::ser::Error> for ZmlError {
    fn from(e: toml::ser::Error) -> Self {
        Self::with_source(ZmlErrorKind::TomlSerialize, e.to_string(), e)
    }
}

/// Result alias for ZML operations.
pub type ZmlResult<T> = Result<T, ZmlError>;
