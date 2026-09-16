//! Schema version 2.x types for `zoid-project.json`.
//!
//! All structs derive `serde::Serialize` and `serde::Deserialize`.  Unknown
//! fields are silently ignored for forward-compatibility: a manifest written by
//! a future version of Zoid can still be loaded by an older binary.

use serde::{Deserialize, Serialize};

use crate::schema::menu_schema::MenuSection;
use crate::schema::theme::ThemeTokens;
use crate::schema::validation::{SchemaValidationError, SchemaValidationResult};

// ── Top-level document ───────────────────────────────────────────────────────

/// A complete `zoid-project.json` document at schema version 2.
///
/// All unknown top-level keys are ignored, enabling forward-compatibility with
/// documents written by newer versions of Zoid.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectV2 {
    /// Human-facing project metadata.
    pub meta: ProjectMeta,

    /// Template reference (which Zoid template this project was generated from).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateRef>,

    /// Feature flags selected for this project.
    #[serde(default)]
    pub features: FeatureSet,

    /// Theme design tokens (colour palette, typography, spacing).
    #[serde(default)]
    pub theme: ThemeTokens,

    /// Output generation configuration.
    #[serde(default)]
    pub output: OutputConfig,

    /// Where Zoid's exporters write the modules they regenerate.
    ///
    /// Recorded at generation time from the template manifest so that export
    /// targets the module the template actually wired into `main.rs`.  Absent
    /// in projects generated before this field existed; the defaults match the
    /// single-crate layout those projects use.
    #[serde(default)]
    pub codegen: CodegenConfig,

    /// Menu structure defined as data (name, items, actions, shortcuts).
    ///
    /// Each entry maps to one top-level menu.  `InWindowMenuBar` and `MenuBar`
    /// builders consume this data model directly via the adapter in
    /// `zoid_gpui::menu::menu_schema_adapter`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menus: Option<Vec<MenuSection>>,

    /// The Zoid version used when this project was generated.
    ///
    /// Set by `zoid create` at generation time.  `zoid check` compares this
    /// against the current Zoid version and warns when the vendored crates may
    /// be out of date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_with: Option<String>,
}

impl ProjectV2 {
    /// Validates all fields and returns the first error found, if any.
    ///
    /// Validation is purely in-memory: it does **not** touch the filesystem.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        self.meta.validate()?;
        if let Some(t) = &self.template {
            t.validate()?;
        }
        self.features.validate()?;
        self.output.validate()?;
        self.codegen.validate()?;
        if let Some(ref menus) = self.menus {
            for section in menus {
                section.validate()?;
            }
        }
        if let Some(ref version) = self.generated_with
            && version.trim().is_empty()
        {
            return Err(SchemaValidationError::empty_field("generated_with"));
        }
        Ok(())
    }
}

// ── Project metadata ─────────────────────────────────────────────────────────

/// Human-facing project metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// Project name (display form).  Must be non-empty.
    pub name: String,

    /// SemVer-compatible project version string (e.g. `"0.1.0"`).
    #[serde(default = "default_project_version")]
    pub version: String,

    /// Short human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// List of authors in `"Name <email>"` or `"Name"` format.
    #[serde(default)]
    pub authors: Vec<String>,
}

fn default_project_version() -> String {
    "0.1.0".to_owned()
}

impl ProjectMeta {
    /// Validates this metadata block.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        if self.name.trim().is_empty() {
            return Err(SchemaValidationError::empty_field("meta.name"));
        }
        if self.version.trim().is_empty() {
            return Err(SchemaValidationError::empty_field("meta.version"));
        }
        Ok(())
    }
}

// ── Template reference ───────────────────────────────────────────────────────

/// A reference to the Zoid template from which this project was generated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TemplateRef {
    /// Template identifier (kebab-case, e.g. `"basic"`).
    pub id: String,

    /// Optional SemVer requirement constraining compatible template versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_req: Option<String>,
}

impl TemplateRef {
    /// Validates the template reference.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        if self.id.trim().is_empty() {
            return Err(SchemaValidationError::empty_field("template.id"));
        }
        if !is_valid_id(&self.id) {
            return Err(SchemaValidationError::invalid_field(
                "template.id",
                "must match ^[a-z][a-z0-9_-]*$",
            ));
        }
        Ok(())
    }
}

// ── Feature set ──────────────────────────────────────────────────────────────

/// The set of Zoid feature flags selected for this project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FeatureSet {
    /// Feature identifiers that are enabled (e.g. `["logging", "config"]`).
    #[serde(default)]
    pub enabled: Vec<String>,
}

impl FeatureSet {
    /// Validates each feature identifier.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        for id in &self.enabled {
            if id.trim().is_empty() {
                return Err(SchemaValidationError::empty_field("features.enabled[]"));
            }
            if !is_valid_id(id) {
                return Err(SchemaValidationError::invalid_field(
                    "features.enabled[]",
                    "each entry must match ^[a-z][a-z0-9_-]*$",
                ));
            }
        }
        Ok(())
    }
}

// ── Output configuration ─────────────────────────────────────────────────────

/// Configuration controlling where and how Zoid writes generated files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Relative or absolute path to the directory where Zoid should write the
    /// generated project.  Must not contain `..` components and must not be an
    /// absolute path beginning with a platform root other than a simple
    /// drive-letter (Windows).
    ///
    /// Defaults to `"."` (current working directory) when absent.
    #[serde(default = "default_output_dir")]
    pub dir: String,

    /// If `true`, existing files in `dir` are overwritten; otherwise Zoid
    /// aborts if the target directory is non-empty.
    #[serde(default)]
    pub overwrite: bool,
}

fn default_output_dir() -> String {
    ".".to_owned()
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            dir: default_output_dir(),
            overwrite: false,
        }
    }
}

impl OutputConfig {
    /// Validates the output configuration.
    ///
    /// Rejects `dir` values that are empty, contain `..` components, or are
    /// absolute paths that look unsafe (bare `/` root without further
    /// components).
    pub fn validate(&self) -> SchemaValidationResult<()> {
        validate_output_dir(&self.dir)
    }
}

// ── Codegen configuration ────────────────────────────────────────────────────

/// Project-root-relative path of the generated element-layout module used when
/// a project records no `codegen` block (every project generated before the
/// block existed, and every single-crate template).
pub const DEFAULT_LAYOUT_PATH: &str = "src/ui/zml_layout.rs";

/// Where Zoid's exporters write the modules they regenerate.
///
/// Each path is relative to the project root and is validated with the same
/// traversal rules as [`OutputConfig::dir`], because a path that reached this
/// struct from a template manifest is untrusted input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodegenConfig {
    /// Element-layout module regenerated from the UI source's node tree.
    ///
    /// Workspace templates place their app crate under `crates/`, so this is
    /// not always `src/ui/zml_layout.rs`.
    #[serde(default = "default_layout_path")]
    pub layout_path: String,

    /// Window-options module regenerated from the UI source's window-level
    /// props.  `None` when the template keeps a hand-written `WindowOptions`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_path: Option<String>,
}

fn default_layout_path() -> String {
    DEFAULT_LAYOUT_PATH.to_owned()
}

impl Default for CodegenConfig {
    fn default() -> Self {
        Self {
            layout_path: default_layout_path(),
            window_path: None,
        }
    }
}

impl CodegenConfig {
    /// Validates both generated-module paths.
    ///
    /// # Errors
    ///
    /// Returns a [`SchemaValidationError`] if either path is empty, contains a
    /// `..` component, or is absolute.
    pub fn validate(&self) -> SchemaValidationResult<()> {
        validate_generated_path("codegen.layout_path", &self.layout_path)?;
        match &self.window_path {
            Some(path) => validate_generated_path("codegen.window_path", path),
            None => Ok(()),
        }
    }
}

/// Validates a project-root-relative path that an exporter will write to.
///
/// Mirrors [`validate_output_dir`]'s rules: no empty values, no `..`
/// components, no absolute paths.  Splitting on both separators catches
/// Windows-style traversal on Unix hosts, where `Path` would treat `..\..` as
/// a single ordinary component.
fn validate_generated_path(field: &str, path: &str) -> SchemaValidationResult<()> {
    if path.trim().is_empty() {
        return Err(SchemaValidationError::empty_field(field));
    }

    for component in path.split(['/', '\\']) {
        if component == ".." {
            return Err(SchemaValidationError::path_traversal(field));
        }
    }

    if std::path::Path::new(path).is_absolute() || path.starts_with('/') {
        return Err(SchemaValidationError::unsafe_path(field));
    }

    Ok(())
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Returns `true` if `id` matches `^[a-z][a-z0-9_-]*$`.
fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Validates a string used as an output directory path.
///
/// Rejected cases:
/// - empty string
/// - contains `..` path components
/// - absolute paths (starting with `/`, `\\`, or a Windows drive root like `C:\`)
pub(crate) fn validate_output_dir(dir: &str) -> SchemaValidationResult<()> {
    if dir.trim().is_empty() {
        return Err(SchemaValidationError::empty_field("output.dir"));
    }

    // Split on both `/` and `\` to catch Windows-style paths.
    for component in dir.split(['/', '\\']) {
        if component == ".." {
            return Err(SchemaValidationError::path_traversal("output.dir"));
        }
    }

    // Reject absolute paths to prevent writes outside the output root.
    // On Windows `Path::is_absolute()` returns false for Unix-style `/…` paths,
    // so we explicitly check for a leading `/` as well.
    if std::path::Path::new(dir).is_absolute() || dir.starts_with('/') {
        return Err(SchemaValidationError::unsafe_path("output.dir"));
    }

    Ok(())
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::schema::validation::SchemaValidationErrorKind;

    #[test]
    fn valid_id_accepts_lower_ascii() {
        assert!(is_valid_id("basic"));
        assert!(is_valid_id("my-feature"));
        assert!(is_valid_id("feat_1"));
    }

    #[test]
    fn valid_id_rejects_uppercase() {
        assert!(!is_valid_id("Basic"));
    }

    #[test]
    fn valid_id_rejects_digit_first() {
        assert!(!is_valid_id("1basic"));
    }

    #[test]
    fn validate_output_dir_rejects_traversal() {
        assert!(validate_output_dir("../evil").is_err());
        assert!(validate_output_dir("a/../../b").is_err());
    }

    #[test]
    fn validate_output_dir_rejects_bare_root() {
        assert!(validate_output_dir("/").is_err());
    }

    #[test]
    fn validate_output_dir_rejects_absolute_path() {
        assert!(validate_output_dir("/etc/passwd").is_err());
        assert!(validate_output_dir("/home/user/secrets").is_err());
        assert!(validate_output_dir("/tmp/evil").is_err());
    }

    #[test]
    fn validate_output_dir_accepts_relative_paths() {
        assert!(validate_output_dir(".").is_ok());
        assert!(validate_output_dir("output/my-project").is_ok());
    }

    // ── Codegen configuration ────────────────────────────────────────────────

    #[test]
    fn codegen_defaults_to_the_single_crate_layout() {
        let codegen = CodegenConfig::default();
        assert_eq!(codegen.layout_path, DEFAULT_LAYOUT_PATH);
        assert_eq!(codegen.window_path, None);
        assert!(codegen.validate().is_ok());
    }

    #[test]
    fn codegen_accepts_a_workspace_layout_path() {
        let codegen = CodegenConfig {
            layout_path: "crates/app/src/ui/zml_layout.rs".to_owned(),
            window_path: Some("crates/app/src/ui/window_options.rs".to_owned()),
        };
        assert!(codegen.validate().is_ok());
    }

    #[test]
    fn codegen_rejects_traversal_in_either_path() {
        let layout = CodegenConfig {
            layout_path: "../outside/zml_layout.rs".to_owned(),
            window_path: None,
        };
        assert_eq!(
            layout.validate().expect_err("traversal").kind(),
            SchemaValidationErrorKind::PathTraversal
        );

        let window = CodegenConfig {
            layout_path: DEFAULT_LAYOUT_PATH.to_owned(),
            window_path: Some("src/../../etc/window_options.rs".to_owned()),
        };
        assert_eq!(
            window.validate().expect_err("traversal").kind(),
            SchemaValidationErrorKind::PathTraversal
        );
    }

    #[test]
    fn codegen_rejects_windows_style_traversal() {
        // `Path::components` on Unix sees `..\..` as one ordinary component, so
        // the separator-aware split is what catches this.
        let codegen = CodegenConfig {
            layout_path: r"..\..\windows\system32\zml_layout.rs".to_owned(),
            window_path: None,
        };
        assert_eq!(
            codegen.validate().expect_err("traversal").kind(),
            SchemaValidationErrorKind::PathTraversal
        );
    }

    #[test]
    fn codegen_rejects_absolute_and_empty_paths() {
        let absolute = CodegenConfig {
            layout_path: "/etc/zml_layout.rs".to_owned(),
            window_path: None,
        };
        assert_eq!(
            absolute.validate().expect_err("absolute").kind(),
            SchemaValidationErrorKind::UnsafePath
        );

        let empty = CodegenConfig {
            layout_path: "   ".to_owned(),
            window_path: None,
        };
        assert_eq!(
            empty.validate().expect_err("empty").kind(),
            SchemaValidationErrorKind::EmptyField
        );
    }

    #[test]
    fn project_without_a_codegen_block_reads_back_as_default() {
        // Every project generated before the block existed looks like this.
        let json = r#"{"schema_version":"2.0.0","meta":{"name":"Legacy"}}"#;
        let project: ProjectV2 = serde_json::from_str(json).expect("parse legacy project");
        assert_eq!(project.codegen.layout_path, DEFAULT_LAYOUT_PATH);
        assert!(project.validate().is_ok());
    }

    #[test]
    fn project_validation_rejects_a_traversing_codegen_path() {
        let project = ProjectV2 {
            // `ProjectMeta::derive(Default)` leaves `version` empty (the serde
            // default only applies when deserializing), so set it explicitly —
            // otherwise `meta` fails first and never reaches `codegen`.
            meta: ProjectMeta {
                name: "App".to_owned(),
                version: "0.1.0".to_owned(),
                ..ProjectMeta::default()
            },
            codegen: CodegenConfig {
                layout_path: "../escape.rs".to_owned(),
                window_path: None,
            },
            ..ProjectV2::default()
        };
        assert_eq!(
            project.validate().expect_err("traversal").kind(),
            SchemaValidationErrorKind::PathTraversal
        );
    }
}
