//! GPUI version inspection, comparison, and migration guidance.
//!
//! This module provides utilities for:
//!
//! - Scanning a project's `Cargo.toml` for its `gpui` dependency version requirement.
//! - Comparing the local requirement against a "latest known" version supplied by a
//!   [`VersionSource`].
//! - Generating human-readable migration guidance when an upgrade is available.
//!
//! Network access is never performed automatically.  Callers must explicitly supply a
//! [`VersionSource`] implementation that fetches remote data (e.g. crates.io); the
//! bundled [`OfflineVersionSource`] always returns `None`.  Test code may use
//! [`MockVersionSource`].

use std::error::Error;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};

// ── Public types ─────────────────────────────────────────────────────────────

/// A validated semver version requirement parsed from a `Cargo.toml` dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuiVersionReq(VersionReq);

impl GpuiVersionReq {
    /// Parses a semver version requirement string.
    ///
    /// # Errors
    ///
    /// Returns [`GpuiVersionErrorKind::Parse`] when the string is not a valid semver
    /// requirement.
    pub fn parse(value: &str) -> Result<Self, GpuiVersionError> {
        VersionReq::parse(value).map(Self).map_err(|source| {
            GpuiVersionError::parse(format!("invalid semver requirement `{value}`"), source)
        })
    }

    /// Returns the inner [`VersionReq`].
    #[must_use]
    pub fn as_req(&self) -> &VersionReq {
        &self.0
    }

    /// Returns the requirement string.
    #[must_use]
    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Display for GpuiVersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A concrete semver version, e.g. the latest published `gpui` version.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct GpuiVersion(Version);

impl GpuiVersion {
    /// Parses a concrete semver version string such as `"0.2.2"`.
    ///
    /// # Errors
    ///
    /// Returns [`GpuiVersionErrorKind::Parse`] when the string is not a valid semver version.
    pub fn parse(value: &str) -> Result<Self, GpuiVersionError> {
        Version::parse(value).map(Self).map_err(|source| {
            GpuiVersionError::parse(format!("invalid semver version `{value}`"), source)
        })
    }

    /// Returns the inner [`Version`].
    #[must_use]
    pub fn as_version(&self) -> &Version {
        &self.0
    }
}

impl Display for GpuiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Relationship between a local version requirement and the latest known version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VersionComparison {
    /// The latest version satisfies the local requirement — no action needed.
    UpToDate,
    /// The latest version does not satisfy the local requirement; an upgrade is available.
    UpdateAvailable {
        /// Current local requirement string (e.g. `"^0.2.1"`).
        local_req: String,
        /// Latest available version (e.g. `"0.3.0"`).
        latest: GpuiVersion,
    },
    /// No latest version is available (offline mode or source returned `None`).
    Unknown,
}

/// Result of scanning a single `Cargo.toml` for a `gpui` dependency.
#[derive(Clone, Debug)]
pub struct ManifestScanResult {
    /// Absolute path to the scanned manifest.
    pub path: PathBuf,
    /// The `gpui` version requirement found, if any.
    pub gpui_req: Option<GpuiVersionReq>,
}

/// Result of checking whether a project's vendored crates match the current
/// Zoid version (the version that generated the project).
///
/// `zoid check` uses this to warn users when vendored sources are outdated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VendorCheckResult {
    /// The project was generated with the same Zoid version — no action needed.
    UpToDate,
    /// The project was generated with an older Zoid version; vendored crates
    /// may be out of date. Contains the version at generation time and the
    /// current version.
    Outdated {
        /// Zoid version recorded at generation time.
        generated: String,
        /// Currently-running Zoid version.
        current: String,
    },
    /// No `zoid-project.json` was found in the project directory.
    NoProjectFile,
    /// A `zoid-project.json` was found but it does not contain a
    /// `generated_with` field.
    NoGeneratedWithField,
}

impl VendorCheckResult {
    /// Returns a human-readable summary string for terminal output.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::UpToDate => "Vendored zoid_gpui: up to date.".to_owned(),
            Self::Outdated {
                generated,
                current,
            } => {
                format!("Vendored zoid_gpui: outdated (generated with Zoid {generated}, current is {current}).")
            }
            Self::NoProjectFile => "Vendored zoid_gpui: no zoid-project.json found — cannot check version.".to_owned(),
            Self::NoGeneratedWithField => "Vendored zoid_gpui: zoid-project.json found but no generated_with field — cannot check version.".to_owned(),
        }
    }

    /// Returns `true` when the vendor check could not determine version status.
    #[must_use]
    pub fn is_indeterminate(&self) -> bool {
        matches!(self, Self::NoProjectFile | Self::NoGeneratedWithField)
    }

    /// Returns `true` when the vendored sources are outdated.
    #[must_use]
    pub fn is_outdated(&self) -> bool {
        matches!(self, Self::Outdated { .. })
    }
}

/// Human-readable migration guidance produced when an upgrade is available.
#[derive(Clone, Debug)]
pub struct MigrationGuidance {
    /// Whether the project's `gpui` dependency is at the latest known version.
    pub is_current: bool,
    /// Short summary line suitable for terminal output.
    pub summary: String,
    /// Detailed guidance paragraphs.
    pub details: Vec<String>,
}

// ── VersionSource trait ───────────────────────────────────────────────────────

/// A source that can supply the latest known `gpui` version.
///
/// Implementations must not perform network I/O unless the caller has explicitly
/// opted in (e.g. by constructing a network-capable source).  The bundled
/// [`OfflineVersionSource`] always returns `None`.
pub trait VersionSource {
    /// Returns the latest known `gpui` version, or `None` if unavailable.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is expected to produce a result but fails
    /// (e.g. a malformed response from crates.io).  Returning `Ok(None)` is the
    /// correct response for offline/unknown scenarios.
    fn latest_gpui_version(&self) -> Result<Option<GpuiVersion>, GpuiVersionError>;
}

/// A [`VersionSource`] that always returns `None` without any network I/O.
///
/// Use this source when offline mode is explicitly requested.
#[derive(Clone, Debug, Default)]
pub struct OfflineVersionSource;

impl VersionSource for OfflineVersionSource {
    fn latest_gpui_version(&self) -> Result<Option<GpuiVersion>, GpuiVersionError> {
        Ok(None)
    }
}

/// A [`VersionSource`] that returns a pre-configured version — for testing only.
#[derive(Clone, Debug)]
pub struct MockVersionSource {
    version: Option<GpuiVersion>,
}

impl MockVersionSource {
    /// Creates a mock source that returns the supplied version.
    #[must_use]
    pub fn with_version(version: GpuiVersion) -> Self {
        Self {
            version: Some(version),
        }
    }

    /// Creates a mock source that returns `None` (simulates unavailable / offline).
    #[must_use]
    pub fn offline() -> Self {
        Self { version: None }
    }
}

impl VersionSource for MockVersionSource {
    fn latest_gpui_version(&self) -> Result<Option<GpuiVersion>, GpuiVersionError> {
        Ok(self.version.clone())
    }
}

// ── Core functions ────────────────────────────────────────────────────────────

/// Scans a `Cargo.toml` at `manifest_path` and extracts the `gpui` dependency version.
///
/// The scanner handles:
/// - `gpui = "0.2.2"` (plain string value)
/// - `gpui = { version = "0.2.2", ... }` (table value with `version` key)
/// - A missing `gpui` dependency (returns `gpui_req: None`)
///
/// # Errors
///
/// Returns [`GpuiVersionErrorKind::Io`] when the file cannot be read, or
/// [`GpuiVersionErrorKind::Manifest`] when the file content is not valid TOML.
///
/// # Security
///
/// Only reads the file at `manifest_path`; does not execute build scripts or
/// evaluate arbitrary manifest fields.  The path is not traversal-checked here —
/// callers must validate user-supplied paths before calling this function.
pub fn scan_manifest(
    manifest_path: impl AsRef<Path>,
) -> Result<ManifestScanResult, GpuiVersionError> {
    let path = manifest_path.as_ref();
    let content = std::fs::read_to_string(path).map_err(|source| {
        GpuiVersionError::io(
            format!("failed to read manifest at `{}`", path.display()),
            source,
        )
    })?;

    let table: toml::Table = content.parse().map_err(|source| {
        GpuiVersionError::manifest(
            format!("invalid TOML in manifest at `{}`", path.display()),
            source,
        )
    })?;

    let gpui_req = extract_gpui_version_req(&table)?;

    Ok(ManifestScanResult {
        path: path.to_path_buf(),
        gpui_req,
    })
}

/// Compares a local [`GpuiVersionReq`] against the latest version from a [`VersionSource`].
///
/// Returns [`VersionComparison::Unknown`] when the source returns `None`.
///
/// # Errors
///
/// Propagates errors from the `VersionSource::latest_gpui_version` call.
pub fn compare_with_source(
    local_req: &GpuiVersionReq,
    source: &dyn VersionSource,
) -> Result<VersionComparison, GpuiVersionError> {
    let latest = source.latest_gpui_version()?;

    Ok(match latest {
        None => VersionComparison::Unknown,
        Some(latest_version) => {
            if local_req.as_req().matches(latest_version.as_version()) {
                VersionComparison::UpToDate
            } else {
                VersionComparison::UpdateAvailable {
                    local_req: local_req.as_str(),
                    latest: latest_version,
                }
            }
        }
    })
}

/// Generates human-readable [`MigrationGuidance`] given a scan result and comparison.
///
/// When the comparison is [`VersionComparison::UpToDate`] the guidance confirms that no
/// action is needed.  When an update is available, the guidance lists the recommended
/// steps and caveats specific to GPUI pre-1.0 volatility.
#[must_use]
pub fn generate_guidance(
    scan: &ManifestScanResult,
    comparison: &VersionComparison,
) -> MigrationGuidance {
    match comparison {
        VersionComparison::Unknown => MigrationGuidance {
            is_current: true,
            summary: format!(
                "{}: gpui dependency found ({}); latest version unknown (offline mode)",
                scan.path.display(),
                scan.gpui_req
                    .as_ref()
                    .map(|r| r.as_str())
                    .unwrap_or_else(|| "(none)".to_owned()),
            ),
            details: vec![
                "Run with an explicit network source to check for newer gpui releases.".to_owned(),
                "See: https://crates.io/crates/gpui".to_owned(),
            ],
        },

        VersionComparison::UpToDate => MigrationGuidance {
            is_current: true,
            summary: format!(
                "{}: gpui {} — up to date",
                scan.path.display(),
                scan.gpui_req
                    .as_ref()
                    .map(|r| r.as_str())
                    .unwrap_or_else(|| "(none)".to_owned()),
            ),
            details: vec![
                "Your gpui dependency satisfies the latest known published version.".to_owned(),
            ],
        },

        VersionComparison::UpdateAvailable { local_req, latest } => {
            let mut details = vec![
                format!(
                    "Your Cargo.toml specifies `gpui = \"{local_req}\"` but gpui {latest} is available."
                ),
                String::new(),
                "Upgrade steps:".to_owned(),
                format!("  1. Update your Cargo.toml: gpui = \"{latest}\""),
                "  2. Run `cargo update` to pull the new version into Cargo.lock.".to_owned(),
                "  3. Run `cargo build` and address any compilation errors.".to_owned(),
                String::new(),
                "GPUI is pre-1.0 and may have breaking API changes between minor versions."
                    .to_owned(),
                "Check the Zed GPUI changelog and docs.rs release notes before upgrading:"
                    .to_owned(),
                format!("  https://docs.rs/gpui/{latest}/gpui/"),
                "  https://github.com/zed-industries/zed/tree/main/crates/gpui".to_owned(),
                String::new(),
                "Consider checking generated templates with `zoid check --compare-zed`".to_owned(),
                "to compare against the gpui version used in the Zed main branch.".to_owned(),
            ];

            // Pre-1.0 major-bump warning
            if latest.as_version().major == 0
                && scan
                    .gpui_req
                    .as_ref()
                    .and_then(minor_from_req)
                    .map(|minor| latest.as_version().minor > minor)
                    .unwrap_or(false)
            {
                details.push(String::new());
                details.push(
                    "⚠  Minor version bump in a pre-1.0 crate may include breaking changes."
                        .to_owned(),
                );
                details.push(
                    "   Review the Zed GPUI commit history for API removals or renames.".to_owned(),
                );
            }

            MigrationGuidance {
                is_current: false,
                summary: format!(
                    "{}: gpui {} → {} (update available)",
                    scan.path.display(),
                    local_req,
                    latest,
                ),
                details,
            }
        }
    }
}

/// Runs a full GPUI version check: scans `manifest_path`, queries `source`, and returns
/// a [`MigrationGuidance`] describing the result.
///
/// This is the primary entry point for the `zoid check gpui` command.
///
/// # Errors
///
/// Returns an error when the manifest cannot be read, is invalid TOML, or the version
/// source fails.
pub fn run_check(
    manifest_path: impl AsRef<Path>,
    source: &dyn VersionSource,
) -> Result<MigrationGuidance, GpuiVersionError> {
    let scan = scan_manifest(manifest_path)?;
    let comparison = match &scan.gpui_req {
        None => VersionComparison::Unknown,
        Some(req) => compare_with_source(req, source)?,
    };
    Ok(generate_guidance(&scan, &comparison))
}

/// Checks whether a project's vendored `zoid_gpui` matches the current Zoid version.
///
/// Reads `zoid-project.json` from `project_dir`, extracts the `generated_with`
/// field, and compares it against `current_version` via semver.
///
/// # Errors
///
/// Returns [`GpuiVersionErrorKind::Io`] when `zoid-project.json` cannot be read,
/// [`GpuiVersionErrorKind::Manifest`] when the file contains invalid JSON, or
/// [`GpuiVersionErrorKind::Vendor`] when the `generated_with` field holds an
/// invalid semver string.
pub fn check_vendor_version(
    project_dir: &Path,
    current_version: &str,
) -> Result<VendorCheckResult, GpuiVersionError> {
    let project_file = project_dir.join("zoid-project.json");
    if !project_file.is_file() {
        return Ok(VendorCheckResult::NoProjectFile);
    }

    let json = std::fs::read_to_string(&project_file).map_err(|source| {
        GpuiVersionError::io(
            format!("failed to read `{}`", project_file.display()),
            source,
        )
    })?;

    // Use a minimal serde_json::Value parse to extract `meta.generated_with`
    // without pulling in the full ProjectV2 type (avoids circular deps and
    // keeps the vendor check self-contained in this module).
    let root: serde_json::Value = serde_json::from_str(&json).map_err(|source| {
        GpuiVersionError::vendor(format!(
            "invalid JSON in `{}`: {source}",
            project_file.display()
        ))
    })?;

    // The JSON uses schema_version-tagged format:
    //   {"schema_version":"2","meta":{...},"generated_with":"3.8.0"}
    // `generated_with` is a top-level field on ProjectV2, flattened by serde tag.
    let generated_with = root
        .as_object()
        .and_then(|obj| obj.get("generated_with"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let Some(generated_str) = generated_with else {
        return Ok(VendorCheckResult::NoGeneratedWithField);
    };

    if generated_str.trim().is_empty() {
        return Ok(VendorCheckResult::NoGeneratedWithField);
    }

    let generated_ver = GpuiVersion::parse(&generated_str).map_err(|_| {
        GpuiVersionError::vendor(format!(
            "invalid semver in generated_with field: `{generated_str}`"
        ))
    })?;

    let current_ver = GpuiVersion::parse(current_version).map_err(|_| {
        GpuiVersionError::vendor(format!("invalid current Zoid version: `{current_version}`"))
    })?;

    if generated_ver == current_ver {
        Ok(VendorCheckResult::UpToDate)
    } else {
        Ok(VendorCheckResult::Outdated {
            generated: generated_str,
            current: current_version.to_owned(),
        })
    }
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors produced by the GPUI version checker.
#[derive(Debug)]
pub struct GpuiVersionError {
    kind: GpuiVersionErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl GpuiVersionError {
    fn parse(message: impl Into<String>, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            kind: GpuiVersionErrorKind::Parse,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    fn io(message: impl Into<String>, source: std::io::Error) -> Self {
        Self {
            kind: GpuiVersionErrorKind::Io,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    fn manifest(message: impl Into<String>, source: toml::de::Error) -> Self {
        Self {
            kind: GpuiVersionErrorKind::Manifest,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Creates a "not found" error for missing manifests or dependencies.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            kind: GpuiVersionErrorKind::NotFound,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a vendor-check-specific error.
    pub fn vendor(message: impl Into<String>) -> Self {
        Self {
            kind: GpuiVersionErrorKind::Vendor,
            message: message.into(),
            source: None,
        }
    }

    /// Returns the machine-readable error kind.
    #[must_use]
    pub fn kind(&self) -> GpuiVersionErrorKind {
        self.kind
    }
}

impl Display for GpuiVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for GpuiVersionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref().map(|s| s as &(dyn Error + 'static))
    }
}

/// Machine-readable error categories for [`GpuiVersionError`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuiVersionErrorKind {
    /// Failed to parse a semver string.
    Parse,
    /// I/O failure reading a manifest file.
    Io,
    /// TOML parse failure in a manifest file.
    Manifest,
    /// The version source returned an error.
    Source,
    /// A required file or dependency was not found.
    NotFound,
    /// An error specific to the vendor-version check.
    Vendor,
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Extracts the `gpui` dependency version requirement from a parsed TOML table.
fn extract_gpui_version_req(
    table: &toml::Table,
) -> Result<Option<GpuiVersionReq>, GpuiVersionError> {
    // Look in [dependencies] and [workspace.dependencies]
    let dep_value = table
        .get("dependencies")
        .and_then(|d| d.as_table())
        .and_then(|t| t.get("gpui"))
        .or_else(|| {
            table
                .get("workspace")
                .and_then(|w| w.as_table())
                .and_then(|w| w.get("dependencies"))
                .and_then(|d| d.as_table())
                .and_then(|t| t.get("gpui"))
        });

    let Some(dep) = dep_value else {
        return Ok(None);
    };

    let version_str = match dep {
        toml::Value::String(s) => s.as_str(),
        toml::Value::Table(t) => match t.get("version").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };

    GpuiVersionReq::parse(version_str).map(Some)
}

/// Attempts to extract the minor version number from the first comparator of a
/// version requirement.  Returns `None` if the requirement has no comparators or
/// the minor is not specified.
fn minor_from_req(req: &GpuiVersionReq) -> Option<u64> {
    req.as_req().comparators.first().and_then(|c| c.minor)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("zoid-gpui-{label}-{nanos}.toml"))
    }

    fn write_temp_manifest(label: &str, content: &str) -> std::path::PathBuf {
        let path = unique_temp_path(label);
        fs::write(&path, content).expect("write temp manifest");
        path
    }

    // ── GpuiVersionReq ────────────────────────────────────────────────────────

    #[test]
    fn version_req_parses_caret() {
        let req = GpuiVersionReq::parse("^0.2.2").expect("valid req");
        assert!(req.as_req().matches(&Version::new(0, 2, 2)));
        assert!(req.as_req().matches(&Version::new(0, 2, 9)));
        assert!(!req.as_req().matches(&Version::new(0, 3, 0)));
    }

    #[test]
    fn version_req_parses_bare_version() {
        let req = GpuiVersionReq::parse("0.2.2").expect("bare version");
        // bare semver means ^0.2.2
        assert!(req.as_req().matches(&Version::new(0, 2, 2)));
    }

    #[test]
    fn version_req_rejects_garbage() {
        let err = GpuiVersionReq::parse("not-a-version").expect_err("should fail");
        assert_eq!(err.kind(), GpuiVersionErrorKind::Parse);
    }

    // ── GpuiVersion ──────────────────────────────────────────────────────────

    #[test]
    fn version_parses_valid() {
        let v = GpuiVersion::parse("0.2.2").expect("valid version");
        assert_eq!(v.as_version().minor, 2);
    }

    #[test]
    fn version_rejects_non_semver() {
        let err = GpuiVersion::parse("v0.2").expect_err("should fail");
        assert_eq!(err.kind(), GpuiVersionErrorKind::Parse);
    }

    #[test]
    fn version_ordering() {
        let a = GpuiVersion::parse("0.2.2").expect("a");
        let b = GpuiVersion::parse("0.3.0").expect("b");
        assert!(b > a);
    }

    // ── scan_manifest ────────────────────────────────────────────────────────

    #[test]
    fn scans_plain_string_dep() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"

[dependencies]
gpui = "0.2.2"
"#;
        let path = write_temp_manifest("plain-string", content);
        let result = scan_manifest(&path).expect("scan");
        fs::remove_file(&path).expect("remove");

        let req = result.gpui_req.expect("gpui dep found");
        assert!(req.as_req().matches(&Version::new(0, 2, 2)));
    }

    #[test]
    fn scans_table_dep() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"

[dependencies]
gpui = { version = "0.2.2", features = ["test-support"] }
"#;
        let path = write_temp_manifest("table-dep", content);
        let result = scan_manifest(&path).expect("scan");
        fs::remove_file(&path).expect("remove");

        let req = result.gpui_req.expect("gpui dep found");
        assert!(req.as_req().matches(&Version::new(0, 2, 2)));
    }

    #[test]
    fn scans_workspace_dep() {
        let content = r#"
[workspace]
members = ["app"]

[workspace.dependencies]
gpui = "0.2.2"
"#;
        let path = write_temp_manifest("workspace-dep", content);
        let result = scan_manifest(&path).expect("scan");
        fs::remove_file(&path).expect("remove");

        let req = result.gpui_req.expect("gpui dep found in workspace");
        assert!(req.as_req().matches(&Version::new(0, 2, 2)));
    }

    #[test]
    fn scans_manifest_without_gpui() {
        let content = r#"
[package]
name = "no-gpui"
version = "0.1.0"

[dependencies]
serde = "1"
"#;
        let path = write_temp_manifest("no-gpui", content);
        let result = scan_manifest(&path).expect("scan");
        fs::remove_file(&path).expect("remove");

        assert!(result.gpui_req.is_none());
    }

    #[test]
    fn scan_rejects_invalid_toml() {
        let content = "this is not [ valid toml";
        let path = write_temp_manifest("invalid", content);
        let err = scan_manifest(&path).expect_err("should fail");
        fs::remove_file(&path).expect("remove");

        assert_eq!(err.kind(), GpuiVersionErrorKind::Manifest);
    }

    #[test]
    fn scan_rejects_missing_file() {
        let err = scan_manifest("/nonexistent/path/to/Cargo.toml").expect_err("should fail");
        assert_eq!(err.kind(), GpuiVersionErrorKind::Io);
    }

    // ── compare_with_source ──────────────────────────────────────────────────

    #[test]
    fn compare_offline_returns_unknown() {
        let req = GpuiVersionReq::parse("0.2.2").expect("req");
        let source = OfflineVersionSource;
        let cmp = compare_with_source(&req, &source).expect("no error");
        assert_eq!(cmp, VersionComparison::Unknown);
    }

    #[test]
    fn compare_up_to_date_when_latest_matches_req() {
        let req = GpuiVersionReq::parse("0.2.2").expect("req");
        let source = MockVersionSource::with_version(GpuiVersion::parse("0.2.2").expect("v"));
        let cmp = compare_with_source(&req, &source).expect("no error");
        assert_eq!(cmp, VersionComparison::UpToDate);
    }

    #[test]
    fn compare_detects_update_available() {
        let req = GpuiVersionReq::parse("0.2.2").expect("req");
        let source = MockVersionSource::with_version(GpuiVersion::parse("0.3.0").expect("v"));
        let cmp = compare_with_source(&req, &source).expect("no error");

        let VersionComparison::UpdateAvailable { latest, .. } = cmp else {
            panic!("expected UpdateAvailable");
        };
        assert_eq!(latest, GpuiVersion::parse("0.3.0").expect("v"));
    }

    #[test]
    fn compare_up_to_date_when_req_satisfied_by_newer_patch() {
        // ^0.2.2 is satisfied by 0.2.9
        let req = GpuiVersionReq::parse("^0.2.2").expect("req");
        let source = MockVersionSource::with_version(GpuiVersion::parse("0.2.9").expect("v"));
        let cmp = compare_with_source(&req, &source).expect("no error");
        assert_eq!(cmp, VersionComparison::UpToDate);
    }

    // ── generate_guidance ────────────────────────────────────────────────────

    #[test]
    fn guidance_up_to_date() {
        let scan = ManifestScanResult {
            path: "/tmp/Cargo.toml".into(),
            gpui_req: Some(GpuiVersionReq::parse("0.2.2").expect("req")),
        };
        let guidance = generate_guidance(&scan, &VersionComparison::UpToDate);
        assert!(guidance.is_current);
        assert!(guidance.summary.contains("up to date"));
    }

    #[test]
    fn guidance_update_available_contains_version_and_steps() {
        let scan = ManifestScanResult {
            path: "/tmp/Cargo.toml".into(),
            gpui_req: Some(GpuiVersionReq::parse("0.2.2").expect("req")),
        };
        let latest = GpuiVersion::parse("0.3.0").expect("v");
        let comparison = VersionComparison::UpdateAvailable {
            local_req: "^0.2.2".to_owned(),
            latest: latest.clone(),
        };
        let guidance = generate_guidance(&scan, &comparison);
        assert!(!guidance.is_current);
        assert!(guidance.summary.contains("update available"));
        let all_details = guidance.details.join("\n");
        assert!(all_details.contains("0.3.0"));
        assert!(all_details.contains("cargo update"));
    }

    #[test]
    fn guidance_minor_bump_warning_included() {
        let scan = ManifestScanResult {
            path: "/tmp/Cargo.toml".into(),
            gpui_req: Some(GpuiVersionReq::parse("0.2.2").expect("req")),
        };
        let latest = GpuiVersion::parse("0.3.0").expect("v");
        let comparison = VersionComparison::UpdateAvailable {
            local_req: "^0.2.2".to_owned(),
            latest,
        };
        let guidance = generate_guidance(&scan, &comparison);
        let all_details = guidance.details.join("\n");
        assert!(all_details.contains("pre-1.0") || all_details.contains("breaking"));
    }

    #[test]
    fn guidance_unknown_when_offline() {
        let scan = ManifestScanResult {
            path: "/tmp/Cargo.toml".into(),
            gpui_req: Some(GpuiVersionReq::parse("0.2.2").expect("req")),
        };
        let guidance = generate_guidance(&scan, &VersionComparison::Unknown);
        assert!(guidance.is_current);
        assert!(guidance.summary.contains("unknown") || guidance.summary.contains("offline"));
    }

    // ── run_check ────────────────────────────────────────────────────────────

    #[test]
    fn run_check_full_flow_offline() {
        let content = r#"
[package]
name = "test-app"
[dependencies]
gpui = "0.2.2"
"#;
        let path = write_temp_manifest("run-check-offline", content);
        let guidance = run_check(&path, &OfflineVersionSource).expect("run_check");
        fs::remove_file(&path).expect("remove");
        assert!(guidance.is_current); // unknown → treated as is_current
    }

    #[test]
    fn run_check_full_flow_with_mock_newer() {
        let content = r#"
[package]
name = "test-app"
[dependencies]
gpui = "0.2.2"
"#;
        let path = write_temp_manifest("run-check-mock-newer", content);
        let source = MockVersionSource::with_version(GpuiVersion::parse("0.3.0").expect("v"));
        let guidance = run_check(&path, &source).expect("run_check");
        fs::remove_file(&path).expect("remove");
        assert!(!guidance.is_current);
        assert!(guidance.summary.contains("update available"));
    }

    // ── vendor version check ───────────────────────────────────────────────

    #[test]
    fn vendor_check_missing_project_file_returns_no_project_file() {
        let dir = temp_dir_for_vendor("missing-project-file");
        let result = check_vendor_version(&dir, "0.1.0").expect("check");
        assert_eq!(result, VendorCheckResult::NoProjectFile);
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn vendor_check_missing_generated_with_field_returns_no_field() {
        let dir = temp_dir_for_vendor("missing-generated_with");
        write_project_json(&dir, r#"{"schema_version":"2","meta":{"name":"Test"}}"#);
        let result = check_vendor_version(&dir, "0.1.0").expect("check");
        assert_eq!(result, VendorCheckResult::NoGeneratedWithField);
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn vendor_check_up_to_date_returns_up_to_date() {
        let dir = temp_dir_for_vendor("up-to-date");
        write_project_json(
            &dir,
            r#"{"schema_version":"2","meta":{"name":"Test"},"generated_with":"0.1.0"}"#,
        );
        let result = check_vendor_version(&dir, "0.1.0").expect("check");
        assert_eq!(result, VendorCheckResult::UpToDate);
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn vendor_check_outdated_returns_outdated() {
        let dir = temp_dir_for_vendor("outdated");
        write_project_json(
            &dir,
            r#"{"schema_version":"2","meta":{"name":"Test"},"generated_with":"0.1.0"}"#,
        );
        let result = check_vendor_version(&dir, "0.2.0").expect("check");
        let VendorCheckResult::Outdated {
            ref generated,
            ref current,
        } = result
        else {
            panic!("expected Outdated, got {result:?}");
        };
        assert_eq!(generated, "0.1.0");
        assert_eq!(current, "0.2.0");
        assert!(result.summary().contains("outdated"));
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn vendor_check_future_version_handled() {
        let dir = temp_dir_for_vendor("future-version");
        write_project_json(
            &dir,
            r#"{"schema_version":"2","meta":{"name":"Test"},"generated_with":"0.3.0"}"#,
        );
        let result = check_vendor_version(&dir, "0.1.0").expect("check");
        // The generated_with is newer than current (user downgraded Zoid).
        // We still treat this as Outdated because the versions differ.
        let VendorCheckResult::Outdated {
            ref generated,
            ref current,
        } = result
        else {
            panic!("expected Outdated for future version, got {result:?}");
        };
        assert_eq!(generated, "0.3.0");
        assert_eq!(current, "0.1.0");
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn vendor_check_empty_generated_with_treated_as_no_field() {
        let dir = temp_dir_for_vendor("empty-generated_with");
        write_project_json(
            &dir,
            r#"{"schema_version":"2","meta":{"name":"Test"},"generated_with":""}"#,
        );
        let result = check_vendor_version(&dir, "0.1.0").expect("check");
        assert_eq!(result, VendorCheckResult::NoGeneratedWithField);
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn vendor_check_invalid_semver_returns_error() {
        let dir = temp_dir_for_vendor("invalid-semver");
        write_project_json(
            &dir,
            r#"{"schema_version":"2","meta":{"name":"Test"},"generated_with":"not-a-version"}"#,
        );
        let result = check_vendor_version(&dir, "0.1.0");
        assert!(result.is_err());
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    // ── test helpers ───────────────────────────────────────────────────────

    fn temp_dir_for_vendor(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("zoid-vendor-check-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_project_json(dir: &Path, content: &str) {
        fs::write(dir.join("zoid-project.json"), content).expect("write project json");
    }
}
