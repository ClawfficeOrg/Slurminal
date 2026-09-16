//! Versioned `zoid-project.json` loader.
//!
//! The top-level [`Project`] enum is discriminated by the `"schema_version"`
//! field in the JSON document.  Callers should use [`Project::from_json`] to
//! load a document and then match on the resulting variant.
//!
//! # Canonical discriminant values
//!
//! | Variant   | `"schema_version"` JSON value |
//! |-----------|-------------------------------|
//! | `V2`      | `"2"`                         |
//!
//! The value is a JSON string (not a number) so that future minor variants
//! like `"2.1"` can be introduced without ambiguity.

use serde::{Deserialize, Serialize};
use serde_json::Error as JsonError;

use crate::schema::project_v2::ProjectV2;

/// A versioned `zoid-project.json` document.
///
/// The active variant is determined at deserialisation time by the
/// `"schema_version"` field.  Documents without that field, or with an
/// unrecognised value, will fail to deserialise with a clear error message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "schema_version")]
pub enum Project {
    /// Schema version 2.x (`"schema_version": "2"`).
    #[serde(rename = "2")]
    V2(ProjectV2),
}

impl Project {
    /// Deserialises a `zoid-project.json` document from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if the input is not valid JSON, if the
    /// `"schema_version"` field is missing, or if its value is not a
    /// recognised version tag.
    pub fn from_json(json: &str) -> Result<Self, JsonError> {
        serde_json::from_str(json)
    }

    /// Serialises this document to a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if serialisation fails (in practice
    /// this should never happen for well-formed data).
    pub fn to_json_pretty(&self) -> Result<String, JsonError> {
        serde_json::to_string_pretty(self)
    }

    /// Returns the contained [`ProjectV2`] reference, if this is a V2 document.
    #[must_use]
    pub fn as_v2(&self) -> Option<&ProjectV2> {
        match self {
            Self::V2(v2) => Some(v2),
        }
    }

    /// Consumes `self` and returns the contained [`ProjectV2`], if V2.
    #[must_use]
    pub fn into_v2(self) -> Option<ProjectV2> {
        match self {
            Self::V2(v2) => Some(v2),
        }
    }
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::schema::project_v2::{FeatureSet, OutputConfig, ProjectMeta};
    use crate::schema::theme::ThemeTokens;

    fn minimal_v2_json() -> &'static str {
        r#"{"schema_version":"2","meta":{"name":"Test"}}"#
    }

    #[test]
    fn deserialize_v2_from_discriminant() {
        let project = Project::from_json(minimal_v2_json()).expect("valid v2 JSON");
        assert!(project.as_v2().is_some());
    }

    #[test]
    fn unknown_schema_version_returns_error() {
        let json = r#"{"schema_version":"99","meta":{"name":"Test"}}"#;
        assert!(Project::from_json(json).is_err());
    }

    #[test]
    fn missing_schema_version_returns_error() {
        let json = r#"{"meta":{"name":"Test"}}"#;
        assert!(Project::from_json(json).is_err());
    }

    #[test]
    fn roundtrip_v2() {
        use crate::schema::project_v2::TemplateRef;

        let original = Project::V2(ProjectV2 {
            meta: ProjectMeta {
                name: "My Project".to_owned(),
                version: "0.1.0".to_owned(),
                description: Some("A test project.".to_owned()),
                authors: vec!["Alice <alice@example.com>".to_owned()],
            },
            template: Some(TemplateRef {
                id: "basic".to_owned(),
                version_req: Some("^1.0".to_owned()),
            }),
            features: FeatureSet {
                enabled: vec!["logging".to_owned()],
            },
            theme: ThemeTokens::default(),
            output: OutputConfig {
                dir: "output/my-project".to_owned(),
                overwrite: false,
            },
            codegen: crate::schema::CodegenConfig {
                layout_path: "crates/app/src/ui/zml_layout.rs".to_owned(),
                window_path: None,
            },
            menus: None,
            generated_with: Some("3.8.2".to_owned()),
        });

        let json = original.to_json_pretty().expect("serialize");
        let loaded = Project::from_json(&json).expect("deserialize");
        assert_eq!(original, loaded);
    }
}
