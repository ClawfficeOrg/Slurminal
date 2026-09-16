//! Version-2 document types produced by the forward migration.
//!
//! These structs represent the canonical in-memory document format after
//! migration.  They are a superset of the v1 types: every v1 field is
//! preserved and new editor-specific fields (`preview`) are added with sane
//! defaults.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A version-2 document — the output of a successful migration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentV2 {
    /// Schema version discriminator — always `2` for this type.
    pub schema_version: u32,
    /// Stable template identifier (kebab-case).
    pub id: String,
    /// Human-readable template name.
    pub name: String,
    /// Human-readable template description.
    pub description: String,
    /// Target platforms.
    #[serde(default)]
    pub platforms: Vec<Platform>,
    /// GPUI dependency source.
    pub gpui: GpuiSource,
    /// Template variables declared by the manifest.
    #[serde(default)]
    pub variables: Vec<Variable>,
    /// Files owned by the template (core files, not feature files).
    #[serde(default)]
    pub files: Vec<FileEntry>,
    /// Feature fragments available in this template.
    #[serde(default)]
    pub features: Vec<Feature>,
    /// Editor preview state — initialised to its default after migration.
    #[serde(default)]
    pub preview: PreviewState,
}

/// Target platform for a v2 document.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    /// macOS target.
    Macos,
    /// Linux target.
    Linux,
    /// Windows target (experimental GPUI support).
    Windows,
}

/// GPUI dependency source for a v2 document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum GpuiSource {
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

/// A template variable declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Variable {
    /// Variable name — must be a valid identifier.
    pub name: String,
    /// Human-readable description shown in the editor UI.
    pub description: String,
    /// Default value used when no override is supplied.
    #[serde(default)]
    pub default: Option<String>,
    /// Whether the user must supply a non-default value before generation.
    #[serde(default)]
    pub required: bool,
}

/// A single file owned by the template.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    /// Path to the source template file (relative to the template root).
    pub source: PathBuf,
    /// Destination path relative to the generated project root.
    pub destination: PathBuf,
    /// How the renderer handles an existing file at `destination`.
    #[serde(default)]
    pub mode: FileMode,
}

/// How the renderer handles a collision when writing a file.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileMode {
    /// Create a new file; fail if the destination already exists.
    #[default]
    Create,
    /// Replace the existing destination file.
    Replace,
    /// Append to a generated section in the destination file.
    Append,
}

/// A feature fragment that can be toggled in the editor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    /// Stable feature identifier (kebab-case).
    pub id: String,
    /// Human-readable feature name.
    pub name: String,
    /// Human-readable feature description.
    pub description: String,
    /// Whether this feature is toggled on by default.
    #[serde(default)]
    pub default: bool,
    /// Feature ids that must also be enabled when this feature is active.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Feature ids that cannot be active at the same time as this feature.
    #[serde(default)]
    pub conflicts: Vec<String>,
    /// Files owned by this feature.
    #[serde(default)]
    pub files: Vec<FileEntry>,
}

/// Editor preview state persisted inside the document.
///
/// After migration this is always initialised to its default value (empty
/// selection, no overrides, not dirty).
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewState {
    /// Feature ids that are currently toggled on in the editor.
    #[serde(default)]
    pub selected_features: Vec<String>,
    /// Per-variable value overrides entered by the user.
    #[serde(default)]
    pub variable_overrides: HashMap<String, String>,
    /// `true` when the document has unsaved changes.
    #[serde(default)]
    pub dirty: bool,
}
