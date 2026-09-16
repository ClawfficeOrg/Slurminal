//! Heuristic scanner that reads an existing Rust/GPUI project and produces
//! a partial [`ZmlDocument`] together with a [`ScanReport`].
//!
//! The scanner reads `.rs` source files as plain text — it never compiles or
//! executes code.  It is best-effort, not a full static analyser.
//!
//! # Recognised GPUI patterns
//!
//! | Pattern ID            | Source trigger                                |
//! |-----------------------|-----------------------------------------------|
//! | `window_lifecycle`    | `App::new()` / `Application::new()`           |
//! | `window_creation`     | `cx.open_window(`                             |
//! | `view_struct`         | `struct …View`                                |
//! | `view_render_impl`    | `impl Render for …`                           |
//! | `view_focusable_impl` | `impl Focusable for …`                        |
//! | `div_layout`          | `div().` chain                                |
//! | `component_derive`    | `#[derive(… IntoElement / RenderOnce …)]`     |
//! | `gpui_entity`         | `cx.new(` / `Entity::<`                       |
//!
//! # Unrecognised items
//!
//! - `use` statements referencing non-GPUI/Zoid/std crates.
//! - `struct` definitions whose names do not end with `View`.

use std::path::{Path, PathBuf};

use crate::schema::{Project, ProjectMeta, ProjectV2};
use crate::zml::{ZML_VERSION, ZmlDocument, ZmlNode};

// ── Public types ──────────────────────────────────────────────────────────────

/// A single finding produced by [`ProjectScanner::scan`].
#[derive(Clone, Debug, PartialEq)]
pub enum ScanFinding {
    /// A GPUI pattern was successfully identified.
    Recognized {
        /// Stable pattern identifier (e.g. `"view_struct"`, `"div_layout"`).
        pattern: String,
        /// Source file path relative to the project root.
        file: String,
        /// 1-indexed line number.
        line: u32,
        /// Truncated line content or extracted name — for display purposes.
        detail: String,
    },
    /// An item was found but could not be mapped to a known GPUI/ZML pattern.
    Unrecognized {
        /// Short description of the item (e.g. `"struct Foo"`).
        item: String,
        /// Human-readable reason for non-recognition.
        reason: String,
        /// Source file path relative to the project root.
        file: String,
        /// 1-indexed line number.
        line: u32,
    },
}

/// Aggregated result of a full project scan.
#[derive(Clone, Debug, Default)]
pub struct ScanReport {
    /// All recognised GPUI patterns.
    pub recognized: Vec<ScanFinding>,
    /// Items found but not mappable to a known pattern.
    pub unrecognized: Vec<ScanFinding>,
    /// Window title extracted from source, if found.
    pub detected_window_title: Option<String>,
    /// Window bounds extracted from source, if found.
    pub detected_bounds: Option<crate::zml::WindowBoundsConfig>,
}

// ── Scanner ───────────────────────────────────────────────────────────────────

/// Heuristically scans an existing Rust/GPUI project directory.
///
/// Create with [`ProjectScanner::new`], then call [`ProjectScanner::scan`] to
/// obtain a [`ScanReport`].
pub struct ProjectScanner {
    /// Canonicalized absolute path to the project root directory.
    root: PathBuf,
}

impl ProjectScanner {
    /// Creates a new scanner for the given project directory.
    ///
    /// # Errors
    ///
    /// Returns an error when `root` does not exist, is not a directory, or
    /// cannot be canonicalized.
    pub fn new(root: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        if !root.exists() {
            return Err(format!("project directory not found: `{}`", root.display()).into());
        }
        if !root.is_dir() {
            return Err(format!("`{}` is not a directory", root.display()).into());
        }
        let canonical = root
            .canonicalize()
            .map_err(|e| format!("cannot canonicalize `{}`: {e}", root.display()))?;
        Ok(Self { root: canonical })
    }

    /// Scans the project directory and returns a [`ScanReport`].
    ///
    /// The scanner first looks for a `src/` subdirectory; if absent it scans
    /// the project root directly.  Only `.rs` files are inspected.
    ///
    /// # Errors
    ///
    /// Returns an error when the source directory cannot be read or a file
    /// cannot be opened.
    pub fn scan(&self) -> Result<ScanReport, Box<dyn std::error::Error>> {
        let src_dir = self.root.join("src");
        let scan_root = if src_dir.is_dir() {
            src_dir
        } else {
            self.root.clone()
        };

        let mut report = ScanReport::default();
        let rs_files = collect_rs_files(&scan_root, &self.root)?;

        for (abs_path, rel_path) in &rs_files {
            let source = std::fs::read_to_string(abs_path)
                .map_err(|e| format!("cannot read `{}`: {e}", abs_path.display()))?;
            scan_source(&source, rel_path, &mut report);
        }

        Ok(report)
    }
}

// ── File walk ─────────────────────────────────────────────────────────────────

/// Recursively collects all `.rs` files under `dir`.
///
/// Returns `(absolute_path, relative_path_string)` pairs.  Symlinks are
/// skipped to prevent path-traversal attacks.
fn collect_rs_files(
    dir: &Path,
    project_root: &Path,
) -> Result<Vec<(PathBuf, String)>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_rs_files_inner(dir, project_root, &mut files)?;
    Ok(files)
}

fn collect_rs_files_inner(
    dir: &Path,
    project_root: &Path,
    files: &mut Vec<(PathBuf, String)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("directory entry error: {e}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| format!("cannot determine file type for `{}`: {e}", path.display()))?;

        // Skip symlinks — no traversal without explicit user consent.
        if file_type.is_symlink() {
            eprintln!("[project-scan] skipping symlink: {}", path.display());
            continue;
        }

        if file_type.is_dir() {
            collect_rs_files_inner(&path, project_root, files)?;
        } else if file_type.is_file() && path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let rel = path
                .strip_prefix(project_root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            files.push((path, rel));
        }
    }

    Ok(())
}

// ── Line scanner ──────────────────────────────────────────────────────────────

/// Scans the full text of one source file and appends findings to `report`.
fn scan_source(source: &str, file_rel: &str, report: &mut ScanReport) {
    for (idx, raw_line) in source.lines().enumerate() {
        let line_num = (idx as u32).saturating_add(1);
        let trimmed = raw_line.trim();

        // Skip blank lines and single-line comments (best-effort).
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        classify_line(trimmed, raw_line, file_rel, line_num, report);
    }
}

/// Classifies a single non-comment source line and appends findings.
fn classify_line(trimmed: &str, raw: &str, file: &str, line: u32, report: &mut ScanReport) {
    let detail = truncate(raw, 120);

    // ── Window lifecycle ──────────────────────────────────────────────────
    if trimmed.contains("App::new()") || trimmed.contains("Application::new()") {
        report.recognized.push(ScanFinding::Recognized {
            pattern: "window_lifecycle".to_string(),
            file: file.to_string(),
            line,
            detail,
        });
        return;
    }

    // ── Window creation ───────────────────────────────────────────────────
    if trimmed.contains("cx.open_window(") || trimmed.contains("cx.open(") {
        // Try to extract a window title from a nearby string literal.
        if let Some(title) = extract_string_literal(trimmed)
            && report.detected_window_title.is_none()
        {
            report.detected_window_title = Some(title);
        }
        report.recognized.push(ScanFinding::Recognized {
            pattern: "window_creation".to_string(),
            file: file.to_string(),
            line,
            detail,
        });
        return;
    }

    // ── Struct definitions ────────────────────────────────────────────────
    let struct_decl = trimmed
        .strip_prefix("pub(crate) ")
        .or_else(|| trimmed.strip_prefix("pub "))
        .unwrap_or(trimmed);
    if struct_decl.starts_with("struct ") {
        let name = extract_ident_after(struct_decl, "struct ");
        if name.ends_with("View") {
            report.recognized.push(ScanFinding::Recognized {
                pattern: "view_struct".to_string(),
                file: file.to_string(),
                line,
                detail: name.to_string(),
            });
        } else {
            report.unrecognized.push(ScanFinding::Unrecognized {
                item: format!("struct {name}"),
                reason: "struct does not match GPUI view naming convention (*View)".to_string(),
                file: file.to_string(),
                line,
            });
        }
        return;
    }

    // ── Render / Focusable impls ──────────────────────────────────────────
    // Heuristic: split on "impl Render for " and take the second segment.
    // This assumes source follows the conventional `impl Render for Name {`
    // form. Malformed input yields empty string and skips classification.
    if trimmed.contains("impl Render for ") {
        let after = trimmed.split("impl Render for ").nth(1).unwrap_or("");
        let name = after
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('{');
        report.recognized.push(ScanFinding::Recognized {
            pattern: "view_render_impl".to_string(),
            file: file.to_string(),
            line,
            detail: name.to_string(),
        });
        return;
    }

    // Heuristic: split on "impl Focusable for " and take the second segment.
    // Same assumption as Render-impl matching above.
    if trimmed.contains("impl Focusable for ") {
        let after = trimmed.split("impl Focusable for ").nth(1).unwrap_or("");
        let name = after
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_end_matches('{');
        report.recognized.push(ScanFinding::Recognized {
            pattern: "view_focusable_impl".to_string(),
            file: file.to_string(),
            line,
            detail: name.to_string(),
        });
        return;
    }

    // ── div() layout trees ────────────────────────────────────────────────
    if trimmed.contains("div().") {
        report.recognized.push(ScanFinding::Recognized {
            pattern: "div_layout".to_string(),
            file: file.to_string(),
            line,
            detail,
        });
        return;
    }

    // ── Component derives ─────────────────────────────────────────────────
    if trimmed.starts_with("#[derive(")
        && (trimmed.contains("IntoElement") || trimmed.contains("RenderOnce"))
    {
        report.recognized.push(ScanFinding::Recognized {
            pattern: "component_derive".to_string(),
            file: file.to_string(),
            line,
            detail,
        });
        return;
    }

    // ── GPUI entity construction ──────────────────────────────────────────
    if trimmed.contains("cx.new(") || trimmed.contains("Entity::<") {
        report.recognized.push(ScanFinding::Recognized {
            pattern: "gpui_entity".to_string(),
            file: file.to_string(),
            line,
            detail,
        });
        return;
    }

    // ── Unrecognised: external use statements ─────────────────────────────
    if trimmed.starts_with("use ") {
        let path = trimmed.trim_start_matches("use ").trim_end_matches(';');
        let is_known = path.starts_with("gpui")
            || path.starts_with("zed")
            || path.starts_with("zoid")
            || path.starts_with("self::")
            || path.starts_with("super::")
            || path.starts_with("crate::")
            || path.starts_with("std::")
            || path.starts_with("core::")
            || path.starts_with("alloc::");
        if !is_known {
            report.unrecognized.push(ScanFinding::Unrecognized {
                item: format!("use {path}"),
                reason: "import from non-GPUI/Zoid crate".to_string(),
                file: file.to_string(),
                line,
            });
        }
    }
}

// ── ZML export ────────────────────────────────────────────────────────────────

/// Converts a [`ScanReport`] into a partial [`ZmlDocument`].
///
/// The root node is always `Window`.  Recognised view structs and render
/// impls become immediate children; recognised `div_layout` fragments are
/// represented as a single `Flex` child node (one per scan, not one per line).
///
/// When no GPUI patterns are found the document is a valid but empty default.
pub fn export_to_zml(report: &ScanReport, project_name: &str) -> ZmlDocument {
    let mut seen_views: Vec<String> = Vec::new();
    let mut children: Vec<ZmlNode> = Vec::new();

    for finding in &report.recognized {
        if let ScanFinding::Recognized {
            pattern, detail, ..
        } = finding
            && (pattern == "view_struct" || pattern == "view_render_impl")
            && !detail.is_empty()
            && !seen_views.contains(detail)
        {
            seen_views.push(detail.clone());
            children.push(ZmlNode {
                component_type: detail.clone(),
                ..ZmlNode::default()
            });
        }
    }

    let div_count = report
        .recognized
        .iter()
        .filter(|f| {
            matches!(
                f,
                ScanFinding::Recognized { pattern, .. } if pattern == "div_layout"
            )
        })
        .count();

    if div_count > 0 {
        children.push(ZmlNode {
            component_type: "Flex".to_string(),
            ..ZmlNode::default()
        });
    }

    let root = ZmlNode {
        component_type: "Window".to_string(),
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
        ..ZmlNode::default()
    };

    ZmlDocument {
        overlays: None,
        zml_version: ZML_VERSION.to_owned(),
        window_title: report
            .detected_window_title
            .clone()
            .or_else(|| Some(project_name.to_string())),
        window_bounds: report.detected_bounds.clone(),
        root,
        theme_overrides: None,
        slots: None,
        bindings: None,
        patterns: None,
    }
}

// ── Write output ──────────────────────────────────────────────────────────────

/// Writes a `zoid-project.json` file into `target_dir` atomically.
///
/// Builds a minimal [`ProjectV2`] using metadata derived from `doc` (window
/// title) and `project_name` as a fallback display name.  Existing files are
/// overwritten.
///
/// # Errors
///
/// Returns an error when serialization or file I/O fails.
pub fn write_zoid_project_json(
    target_dir: &Path,
    doc: &ZmlDocument,
    project_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_name = doc
        .window_title
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(project_name);

    let name = if display_name.trim().is_empty() {
        "imported-project".to_string()
    } else {
        display_name.to_string()
    };

    let project = Project::V2(ProjectV2 {
        meta: ProjectMeta {
            name,
            version: "0.1.0".to_string(),
            description: Some(
                "Imported from existing Rust/GPUI project via `zoid import`.".to_string(),
            ),
            authors: Vec::new(),
        },
        ..ProjectV2::default()
    });

    let json = project
        .to_json_pretty()
        .map_err(|e| format!("cannot serialize project: {e}"))?;

    let output_path = target_dir.join("zoid-project.json");
    atomic_write(&output_path, &json)?;

    Ok(())
}

// ── Report printer ────────────────────────────────────────────────────────────

/// Prints the scan report to stdout.
///
/// When `verbose` is `false` only a summary line and an unrecognised-item
/// count are printed.  When `true`, every individual finding is listed.
pub fn emit_report(report: &ScanReport, verbose: bool) {
    println!(
        "[scan] recognized: {}, unrecognized: {}",
        report.recognized.len(),
        report.unrecognized.len(),
    );

    if verbose {
        for finding in &report.recognized {
            if let ScanFinding::Recognized {
                pattern,
                file,
                line,
                detail,
            } = finding
            {
                println!("[recognized] {pattern} — {file}:{line} — {detail}");
            }
        }
        for finding in &report.unrecognized {
            if let ScanFinding::Unrecognized {
                item,
                reason,
                file,
                line,
            } = finding
            {
                println!("[unrecognized] {item} — {file}:{line} — {reason}");
            }
        }
    } else if !report.unrecognized.is_empty() {
        println!(
            "    {} unrecognized item(s) — rerun with --verbose for details",
            report.unrecognized.len()
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Truncates `s` to at most `max_chars` Unicode scalar values, appending `…`
/// when truncated.
fn truncate(s: &str, max_chars: usize) -> String {
    let mut chars = s.char_indices();
    match chars.nth(max_chars) {
        Some((byte_idx, _)) => format!("{}…", &s[..byte_idx]),
        None => s.to_string(),
    }
}

/// Returns the first identifier (word) that appears after `prefix` in `s`.
///
/// An identifier contains only ASCII alphanumerics and underscores.
fn extract_ident_after<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix)
        .unwrap_or("")
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("")
}

/// Extracts the first non-empty double-quoted string literal from `s`.
fn extract_string_literal(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let rest = &s[start..];
    let end = rest.find('"')?;
    let value = &rest[..end];
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Writes `content` to `path` atomically via a temporary file and rename.
fn atomic_write(path: &Path, content: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content).map_err(|e| format!("cannot write `{}`: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        format!(
            "cannot rename `{}` to `{}`: {e}",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // ── truncate ──────────────────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_string_appends_ellipsis() {
        let result = truncate("hello world", 5);
        assert_eq!(result, "hello…");
    }

    // ── extract_ident_after ───────────────────────────────────────────────

    #[test]
    fn extract_ident_basic() {
        assert_eq!(extract_ident_after("struct MyView {", "struct "), "MyView");
    }

    #[test]
    fn extract_ident_generic() {
        assert_eq!(
            extract_ident_after("struct GenericView<T> {", "struct "),
            "GenericView"
        );
    }

    // ── extract_string_literal ────────────────────────────────────────────

    #[test]
    fn extract_string_literal_finds_first() {
        assert_eq!(
            extract_string_literal(r#"title: "My App","#),
            Some("My App".to_string())
        );
    }

    #[test]
    fn extract_string_literal_none_when_empty() {
        assert_eq!(extract_string_literal(r#"title: "","#), None);
    }

    // ── scan_source ───────────────────────────────────────────────────────

    fn scan(source: &str) -> ScanReport {
        let mut report = ScanReport::default();
        scan_source(source, "test.rs", &mut report);
        report
    }

    #[test]
    fn scan_recognizes_window_lifecycle() {
        let report = scan("    App::new().run(|cx| {});");
        assert_eq!(report.recognized.len(), 1);
        if let ScanFinding::Recognized { pattern, .. } = &report.recognized[0] {
            assert_eq!(pattern, "window_lifecycle");
        } else {
            panic!("expected Recognized");
        }
    }

    #[test]
    fn scan_recognizes_application_new() {
        let report = scan("    Application::new().run(cx);");
        assert_eq!(report.recognized.len(), 1);
        if let ScanFinding::Recognized { pattern, .. } = &report.recognized[0] {
            assert_eq!(pattern, "window_lifecycle");
        } else {
            panic!("expected Recognized");
        }
    }

    #[test]
    fn scan_recognizes_window_creation() {
        let report = scan("    cx.open_window(options, |cx| {});");
        let found = report
            .recognized
            .iter()
            .any(|f| matches!(f, ScanFinding::Recognized { pattern, .. } if pattern == "window_creation"));
        assert!(found, "window_creation not recognized");
    }

    #[test]
    fn scan_recognizes_view_struct() {
        let report = scan("pub struct AppView {}");
        assert!(report.recognized.iter().any(|f| {
            matches!(f, ScanFinding::Recognized { pattern, detail, .. }
                if pattern == "view_struct" && detail == "AppView")
        }));
    }

    #[test]
    fn scan_recognizes_render_impl() {
        let report = scan("impl Render for AppView {");
        assert!(report.recognized.iter().any(|f| {
            matches!(f, ScanFinding::Recognized { pattern, .. } if pattern == "view_render_impl")
        }));
    }

    #[test]
    fn scan_recognizes_div_layout() {
        let report = scan("        div().flex().gap(px(8)).child(label)");
        assert!(report.recognized.iter().any(|f| {
            matches!(f, ScanFinding::Recognized { pattern, .. } if pattern == "div_layout")
        }));
    }

    #[test]
    fn scan_recognizes_component_derive() {
        let report = scan("#[derive(Clone, IntoElement)]");
        assert!(report.recognized.iter().any(|f| {
            matches!(f, ScanFinding::Recognized { pattern, .. } if pattern == "component_derive")
        }));
    }

    #[test]
    fn scan_recognizes_gpui_entity() {
        let report = scan("    let model = cx.new(|_| MyModel::default());");
        assert!(report.recognized.iter().any(|f| {
            matches!(f, ScanFinding::Recognized { pattern, .. } if pattern == "gpui_entity")
        }));
    }

    #[test]
    fn scan_marks_non_view_struct_unrecognized() {
        let report = scan("struct AppConfig {}");
        assert!(report.unrecognized.iter().any(|f| {
            matches!(f, ScanFinding::Unrecognized { item, .. } if item.contains("AppConfig"))
        }));
    }

    #[test]
    fn scan_marks_external_use_unrecognized() {
        let report = scan("use serde::Serialize;");
        assert!(report.unrecognized.iter().any(|f| {
            matches!(f, ScanFinding::Unrecognized { reason, .. }
                if reason.contains("non-GPUI"))
        }));
    }

    #[test]
    fn scan_skips_comment_lines() {
        let report = scan("// App::new().run(|cx| {});");
        assert!(report.recognized.is_empty());
        assert!(report.unrecognized.is_empty());
    }

    #[test]
    fn scan_known_use_not_unrecognized() {
        let report = scan("use gpui::App;");
        assert!(report.unrecognized.is_empty());
    }

    // ── export_to_zml ─────────────────────────────────────────────────────

    #[test]
    fn export_to_zml_uses_detected_title() {
        let report = ScanReport {
            detected_window_title: Some("My App".to_string()),
            ..ScanReport::default()
        };
        let doc = export_to_zml(&report, "fallback");
        assert_eq!(doc.window_title.as_deref(), Some("My App"));
    }

    #[test]
    fn export_to_zml_falls_back_to_project_name() {
        let report = ScanReport::default();
        let doc = export_to_zml(&report, "my-project");
        assert_eq!(doc.window_title.as_deref(), Some("my-project"));
    }

    #[test]
    fn export_to_zml_empty_project_valid() {
        let report = ScanReport::default();
        let doc = export_to_zml(&report, "empty");
        assert_eq!(doc.root.component_type, "Window");
        assert!(doc.root.children.is_none());
    }

    #[test]
    fn export_to_zml_adds_view_children() {
        let mut report = ScanReport::default();
        report.recognized.push(ScanFinding::Recognized {
            pattern: "view_struct".to_string(),
            file: "src/main.rs".to_string(),
            line: 1,
            detail: "AppView".to_string(),
        });
        let doc = export_to_zml(&report, "proj");
        let children = doc.root.children.as_ref().expect("children");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].component_type, "AppView");
    }

    #[test]
    fn export_to_zml_deduplicates_views() {
        let mut report = ScanReport::default();
        for _ in 0..3 {
            report.recognized.push(ScanFinding::Recognized {
                pattern: "view_render_impl".to_string(),
                file: "src/main.rs".to_string(),
                line: 1,
                detail: "AppView".to_string(),
            });
        }
        let doc = export_to_zml(&report, "proj");
        let children = doc.root.children.as_ref().expect("children");
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn export_to_zml_adds_flex_for_div_layouts() {
        let mut report = ScanReport::default();
        report.recognized.push(ScanFinding::Recognized {
            pattern: "div_layout".to_string(),
            file: "src/main.rs".to_string(),
            line: 5,
            detail: "div().flex()".to_string(),
        });
        let doc = export_to_zml(&report, "proj");
        let children = doc.root.children.as_ref().expect("children");
        assert!(children.iter().any(|n| n.component_type == "Flex"));
    }
}
