//! Version-1 document types used as migration input.
//!
//! These structs mirror the initial Zoid template manifest format encoded as
//! JSON with an explicit `schema_version: 1` discriminator.  They are
//! read-only from the migration perspective; the migration function consumes
//! them and produces a [`super::v2::DocumentV2`].

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A version-1 document, as serialised to disk before schema version 2.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentV1 {
    /// Schema version discriminator — must be `1`.
    pub schema_version: u32,
    /// Manifest schema semver string (e.g. `"0.1.0"`).
    ///
    /// This field is **lossy**: it is present in v1 but has no counterpart in
    /// v2.  The migration emits a warning log and drops this value.
    pub manifest_version: String,
    /// Stable template identifier (kebab-case).
    pub id: String,
    /// Human-readable template name.
    pub name: String,
    /// Human-readable template description.
    pub description: String,
    /// Target platforms.
    #[serde(default)]
    pub platforms: Vec<PlatformV1>,
    /// GPUI dependency source.
    pub gpui: GpuiSourceV1,
    /// Template variables declared by the manifest.
    #[serde(default)]
    pub variables: Vec<VariableV1>,
    /// Files owned by the template.
    #[serde(default)]
    pub files: Vec<FileEntryV1>,
    /// Optional feature fragments declared by the template.
    #[serde(default)]
    pub features: Vec<FeatureV1>,
}

/// Target platform for a v1 document.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlatformV1 {
    /// macOS target.
    Macos,
    /// Linux target.
    Linux,
    /// Windows target.
    Windows,
}

/// GPUI dependency source for a v1 document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum GpuiSourceV1 {
    /// Published crates.io GPUI crate.
    CratesIo {
        /// SemVer requirement string (e.g. `"^0.2"`).
        version: String,
    },
    /// Pinned Zed Git revision.
    ZedGit {
        /// Git URL for the Zed monorepo.
        repository: String,
        /// Commit SHA or stable ref.
        rev: String,
    },
}

/// Variable declaration in a v1 document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariableV1 {
    /// Variable name (must be a valid identifier).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Default value, if any.
    #[serde(default)]
    pub default: Option<String>,
    /// Whether the variable must be supplied by the user.
    #[serde(default)]
    pub required: bool,
}

/// File declaration in a v1 document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntryV1 {
    /// Source template path.
    pub source: PathBuf,
    /// Destination path relative to the generated project root.
    pub destination: PathBuf,
    /// Write mode.
    #[serde(default)]
    pub mode: FileModeV1,
}

/// File write mode in a v1 document.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileModeV1 {
    /// Create a new file; fail on collision.
    #[default]
    Create,
    /// Replace an existing generated file.
    Replace,
    /// Append to a generated section.
    Append,
}

/// Feature fragment declaration in a v1 document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureV1 {
    /// Stable feature identifier (kebab-case).
    pub id: String,
    /// Human-readable feature name.
    pub name: String,
    /// Human-readable feature description.
    pub description: String,
    /// Whether this feature is enabled by default.
    #[serde(default)]
    pub default: bool,
    /// Required feature identifiers.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Conflicting feature identifiers.
    #[serde(default)]
    pub conflicts: Vec<String>,
    /// Files owned by this feature.
    #[serde(default)]
    pub files: Vec<FileEntryV1>,
}
