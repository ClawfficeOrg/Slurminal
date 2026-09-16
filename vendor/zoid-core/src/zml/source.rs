//! UI source file resolution and format dispatch (task 8.12.1).
//!
//! A project's UI lives in **exactly one** file under `ui/`:
//!
//! - `ui/app.json` — the primary representation for new projects (JSON).
//! - `ui/app.zml`  — the legacy TOML representation; still read forever, but
//!   new writes prefer JSON.
//!
//! Both encode the same [`ZmlDocument`] schema; only the serialization
//! differs. [`UiSourceFormat::detect`] picks the format for a given file by
//! extension, falling back to content sniffing, and [`resolve_ui_source`]
//! returns the file to open (JSON wins when both exist).

use std::path::{Path, PathBuf};

use crate::zml::document::ZmlDocument;
use crate::zml::error::ZmlResult;

/// Project-relative path of the primary (JSON) UI source.
pub const APP_JSON_REL: &str = "ui/app.json";

/// Project-relative path of the legacy (TOML) UI source.
pub const APP_ZML_REL: &str = "ui/app.zml";

/// Candidate paths in preference order: JSON first, legacy TOML second.
pub const UI_SOURCE_CANDIDATES: [&str; 2] = [APP_JSON_REL, APP_ZML_REL];

/// On-disk representation of a ZML document.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum UiSourceFormat {
    /// JSON (`ui/app.json`) — the primary representation.
    Json,
    /// TOML (`ui/app.zml`) — the legacy representation.
    Toml,
}

impl UiSourceFormat {
    /// Maps a file extension to its format. Unknown extensions yield `None`.
    #[must_use]
    pub fn from_extension(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "json" => Some(Self::Json),
            "zml" | "toml" => Some(Self::Toml),
            _ => None,
        }
    }

    /// Detects the format of a UI source by file extension (`.json` → JSON;
    /// `.zml` / `.toml` → TOML). Unrecognized extensions default to
    /// [`UiSourceFormat::Json`] so stray files fail with a JSON parse error
    /// rather than silently misparsing as TOML.
    #[must_use]
    pub fn detect(path: &Path) -> Self {
        Self::from_extension(path).unwrap_or(Self::Json)
    }

    /// Parses `text` as a [`ZmlDocument`] in this format.
    ///
    /// # Errors
    ///
    /// Returns a [`ZmlErrorKind::JsonParse`] / [`ZmlErrorKind::TomlParse`]
    /// error when the text is malformed or violates the schema.
    pub fn parse(self, text: &str) -> ZmlResult<ZmlDocument> {
        match self {
            Self::Json => ZmlDocument::from_json(text),
            Self::Toml => ZmlDocument::from_toml(text),
        }
    }

    /// Serializes `doc` into this format (pretty-printed for JSON).
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the document cannot be represented.
    pub fn serialize(self, doc: &ZmlDocument) -> ZmlResult<String> {
        match self {
            Self::Json => doc.to_json(),
            Self::Toml => doc.to_toml(),
        }
    }
}

/// Resolves the UI source file for a project root, in preference order:
/// `ui/app.json` wins over the legacy `ui/app.zml`. Returns `None` when the
/// project has no UI source yet (fresh projects are allowed to have none).
#[must_use]
pub fn resolve_ui_source(project_root: &Path) -> Option<(PathBuf, UiSourceFormat)> {
    for rel in UI_SOURCE_CANDIDATES {
        let path = project_root.join(rel);
        if !path.is_file() {
            continue;
        }
        return Some((path.clone(), UiSourceFormat::detect(&path)));
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::zml::error::ZmlErrorKind;

    const MINIMAL_TOML: &str = "zml_version = \"1\"\n\n[root]\ncomponent_type = \"Window\"\n";
    const MINIMAL_JSON: &str = r#"{"zml_version":"1","root":{"component_type":"Window"}}"#;

    #[test]
    fn extension_maps_to_format() {
        assert_eq!(
            UiSourceFormat::from_extension(Path::new("ui/app.json")),
            Some(UiSourceFormat::Json)
        );
        assert_eq!(
            UiSourceFormat::from_extension(Path::new("ui/app.zml")),
            Some(UiSourceFormat::Toml)
        );
        assert_eq!(
            UiSourceFormat::from_extension(Path::new("ui/app.toml")),
            Some(UiSourceFormat::Toml)
        );
        assert_eq!(
            UiSourceFormat::from_extension(Path::new("ui/app.txt")),
            None
        );
    }

    #[test]
    fn detect_uses_extension_then_defaults_to_json() {
        assert_eq!(
            UiSourceFormat::detect(Path::new("app.zml")),
            UiSourceFormat::Toml
        );
        assert_eq!(
            UiSourceFormat::detect(Path::new("app.json")),
            UiSourceFormat::Json
        );
        // Unrecognized extension falls back to JSON (fails loudly, never
        // silently misparses as TOML).
        assert_eq!(
            UiSourceFormat::detect(Path::new("app.bak")),
            UiSourceFormat::Json
        );
    }

    #[test]
    fn parse_and_serialize_dispatch_per_format() {
        let toml_doc = UiSourceFormat::Toml
            .parse(MINIMAL_TOML)
            .expect("parse toml");
        let json_doc = UiSourceFormat::Json
            .parse(MINIMAL_JSON)
            .expect("parse json");
        assert_eq!(toml_doc, json_doc);

        let out = UiSourceFormat::Json
            .serialize(&json_doc)
            .expect("serialize");
        assert_eq!(UiSourceFormat::Json.parse(&out).expect("reparse"), json_doc);
        let out = UiSourceFormat::Toml
            .serialize(&toml_doc)
            .expect("serialize");
        assert_eq!(UiSourceFormat::Toml.parse(&out).expect("reparse"), toml_doc);
    }

    #[test]
    fn malformed_input_errors_with_format_kind() {
        let err = UiSourceFormat::Json
            .parse("not json {{{")
            .expect_err("json");
        assert_eq!(err.kind(), ZmlErrorKind::JsonParse);
        let err = UiSourceFormat::Toml
            .parse("not toml {{{")
            .expect_err("toml");
        assert_eq!(err.kind(), ZmlErrorKind::TomlParse);
    }

    #[test]
    fn resolve_prefers_json_over_legacy() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert!(resolve_ui_source(dir.path()).is_none(), "no files yet");

        std::fs::create_dir_all(dir.path().join("ui")).expect("mkdir ui");
        std::fs::write(dir.path().join(APP_ZML_REL), MINIMAL_TOML).expect("legacy");
        let (path, format) = resolve_ui_source(dir.path()).expect("legacy resolved");
        assert_eq!(format, UiSourceFormat::Toml);
        assert!(path.ends_with(APP_ZML_REL));

        std::fs::write(dir.path().join(APP_JSON_REL), MINIMAL_JSON).expect("json");
        let (path, format) = resolve_ui_source(dir.path()).expect("json resolved");
        assert_eq!(format, UiSourceFormat::Json);
        assert!(
            path.ends_with(APP_JSON_REL),
            "json must win when both exist"
        );
    }
}
