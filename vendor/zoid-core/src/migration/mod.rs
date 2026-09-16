//! Forward migration from schema version 1 to version 2.
//!
//! # Overview
//!
//! This module provides [`migrate_v1_to_v2`], the canonical forward-migration
//! function for Zoid editor documents.  It accepts a [`v1::DocumentV1`] and
//! returns a [`v2::DocumentV2`], or a typed [`MigrationError`] on failure.
//!
//! ## Lossy fields
//!
//! The `manifest_version` field present in v1 documents has no counterpart in
//! v2.  When this field contains a non-empty value the migration emits a
//! `warn`-level log message so that callers can surface the information loss
//! to the user.
//!
//! ## Optional fields and defaults
//!
//! All optional collections (`platforms`, `variables`, `files`, `features`)
//! default to empty `Vec`s via `#[serde(default)]` on the v1 types.
//! `mode`, `required`, and feature `default` likewise have sane defaults.
//! The v2 `preview` state is always initialised to [`v2::PreviewState::default`].
//!
//! ## Validation
//!
//! The following invariants are checked before migration proceeds; violation
//! returns [`MigrationErrorKind::InvalidInput`]:
//!
//! - `schema_version` must equal `1`
//! - `id`, `name`, and `description` must not be blank
//! - Every variable must have a non-blank `name`
//! - Every feature must have a non-blank `id`

pub mod v1;
pub mod v2;

use v1::{DocumentV1, FeatureV1, FileEntryV1, FileModeV1, GpuiSourceV1, PlatformV1, VariableV1};
use v2::{DocumentV2, Feature, FileEntry, FileMode, GpuiSource, Platform, PreviewState, Variable};

use std::error::Error;
use std::fmt::{self, Display};

/// Result alias used throughout the migration module.
pub type MigrationResult<T> = Result<T, MigrationError>;

// ─── error types ─────────────────────────────────────────────────────────────

/// All possible failures that can occur during a v1 → v2 migration.
#[derive(Debug)]
pub struct MigrationError {
    kind: MigrationErrorKind,
    message: String,
}

impl MigrationError {
    /// Creates a new error.
    pub(crate) fn new(kind: MigrationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Returns the machine-readable error kind.
    #[must_use]
    pub fn kind(&self) -> MigrationErrorKind {
        self.kind
    }
}

impl Display for MigrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for MigrationError {}

/// Machine-readable categories for [`MigrationError`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationErrorKind {
    /// The input document's `schema_version` is not `1`.
    WrongVersion,
    /// A required field was blank or missing.
    InvalidInput,
}

// ─── public migration entry point ────────────────────────────────────────────

/// Migrates a version-1 document to version 2.
///
/// # Behaviour
///
/// - All v1 fields that have a v2 equivalent are copied verbatim.
/// - The `manifest_version` field is **dropped** (it has no v2 counterpart).
///   If the value is non-empty a `warn`-level log message is emitted.
/// - The v2 `preview` state is initialised to [`PreviewState::default`].
/// - Missing optional collections default to empty `Vec`.
///
/// # Errors
///
/// Returns [`MigrationErrorKind::WrongVersion`] when `source.schema_version`
/// is not `1`, and [`MigrationErrorKind::InvalidInput`] when a required field
/// is blank.
pub fn migrate_v1_to_v2(source: DocumentV1) -> MigrationResult<DocumentV2> {
    validate_v1(&source)?;

    // `manifest_version` is the only lossy field: it exists in v1 but has no
    // v2 counterpart.  Emit a warning so callers can surface this to the user.
    if !source.manifest_version.trim().is_empty() {
        log::warn!(
            "migration v1→v2: dropping lossy field `manifest_version` = {:?}; \
             this value has no equivalent in schema version 2",
            source.manifest_version,
        );
    }

    Ok(DocumentV2 {
        schema_version: 2,
        id: source.id,
        name: source.name,
        description: source.description,
        platforms: source.platforms.into_iter().map(migrate_platform).collect(),
        gpui: migrate_gpui_source(source.gpui),
        variables: source.variables.into_iter().map(migrate_variable).collect(),
        files: source.files.into_iter().map(migrate_file_entry).collect(),
        features: source.features.into_iter().map(migrate_feature).collect(),
        preview: PreviewState::default(),
    })
}

// ─── validation ──────────────────────────────────────────────────────────────

/// Validates invariants on a v1 document that cannot be expressed in serde.
fn validate_v1(doc: &DocumentV1) -> MigrationResult<()> {
    if doc.schema_version != 1 {
        return Err(MigrationError::new(
            MigrationErrorKind::WrongVersion,
            format!(
                "expected schema_version 1 for v1→v2 migration, got {}",
                doc.schema_version
            ),
        ));
    }
    require_non_blank("id", &doc.id)?;
    require_non_blank("name", &doc.name)?;
    require_non_blank("description", &doc.description)?;
    for variable in &doc.variables {
        require_non_blank("variable.name", &variable.name)?;
    }
    for feature in &doc.features {
        require_non_blank("feature.id", &feature.id)?;
    }
    Ok(())
}

fn require_non_blank(field: &str, value: &str) -> MigrationResult<()> {
    if value.trim().is_empty() {
        return Err(MigrationError::new(
            MigrationErrorKind::InvalidInput,
            format!("required field `{field}` must not be blank"),
        ));
    }
    Ok(())
}

// ─── field-level conversions ─────────────────────────────────────────────────

fn migrate_platform(platform: PlatformV1) -> Platform {
    match platform {
        PlatformV1::Macos => Platform::Macos,
        PlatformV1::Linux => Platform::Linux,
        PlatformV1::Windows => Platform::Windows,
    }
}

fn migrate_gpui_source(source: GpuiSourceV1) -> GpuiSource {
    match source {
        GpuiSourceV1::CratesIo { version } => GpuiSource::CratesIo { version },
        GpuiSourceV1::ZedGit { repository, rev } => GpuiSource::ZedGit { repository, rev },
    }
}

fn migrate_variable(variable: VariableV1) -> Variable {
    Variable {
        name: variable.name,
        description: variable.description,
        default: variable.default,
        required: variable.required,
    }
}

fn migrate_file_entry(file: FileEntryV1) -> FileEntry {
    FileEntry {
        source: file.source,
        destination: file.destination,
        mode: migrate_file_mode(file.mode),
    }
}

fn migrate_file_mode(mode: FileModeV1) -> FileMode {
    match mode {
        FileModeV1::Create => FileMode::Create,
        FileModeV1::Replace => FileMode::Replace,
        FileModeV1::Append => FileMode::Append,
    }
}

fn migrate_feature(feature: FeatureV1) -> Feature {
    Feature {
        id: feature.id,
        name: feature.name,
        description: feature.description,
        default: feature.default,
        dependencies: feature.dependencies,
        conflicts: feature.conflicts,
        files: feature.files.into_iter().map(migrate_file_entry).collect(),
    }
}
