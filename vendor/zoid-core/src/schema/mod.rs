//! Schema types and versioned loader for `zoid-project.json`.
//!
//! ## Quick start
//!
//! ```no_run
//! use zoid_core::schema::Project;
//!
//! let json = std::fs::read_to_string("zoid-project.json").unwrap();
//! let project = Project::from_json(&json).unwrap();
//!
//! match project {
//!     Project::V2(v2) => {
//!         v2.validate().unwrap();
//!         println!("Project name: {}", v2.meta.name);
//!     }
//! }
//! ```

pub mod menu_schema;
pub mod project;
pub mod project_v2;
pub mod theme;
pub mod validation;

pub use menu_schema::{MenuBarDef, MenuItemEntry, MenuItemSchema, MenuSection, MenuShortcut};
pub use project::Project;
pub use project_v2::{
    CodegenConfig, DEFAULT_LAYOUT_PATH, FeatureSet, OutputConfig, ProjectMeta, ProjectV2,
    TemplateRef,
};
#[allow(deprecated)]
pub use theme::{
    ColorPalette, ColorTokens, LegacyThemeTokens, MotionTokens, RadiusTokens, ShadowSet,
    ShadowTokens, SpacingTokens, ThemeTokens, TypographyTokens,
};
pub use validation::{SchemaValidationError, SchemaValidationErrorKind, SchemaValidationResult};
