//! Import tooling: scan an existing Rust/GPUI project and produce a partial
//! ZML document plus `zoid-project.json` metadata.
//!
//! # Quick start
//!
//! ```no_run
//! use std::path::PathBuf;
//! use zoid_core::import::project::{
//!     ProjectScanner, emit_report, export_to_zml, write_zoid_project_json,
//! };
//!
//! let scanner = ProjectScanner::new(PathBuf::from("/path/to/project")).unwrap();
//! let report  = scanner.scan().unwrap();
//! let doc     = export_to_zml(&report, "my-project");
//! write_zoid_project_json(
//!     std::path::Path::new("/path/to/project"), &doc, "my-project"
//! ).unwrap();
//! emit_report(&report, false);
//! ```

pub mod project;

pub use project::{
    ProjectScanner, ScanFinding, ScanReport, emit_report, export_to_zml, write_zoid_project_json,
};
