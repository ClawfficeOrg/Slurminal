//! Full Zoid design-token schema for the `theme` section of `zoid-project.json`.
//!
//! # Format history
//!
//! | Tag | Description | Status |
//! |-----|-------------|--------|
//! | 1.3.3 flat | `HashMap<String, String>` of dotted keys | **Deprecated** — readable via automatic migration |
//! | 2.0 structured | Typed token groups with light/dark variants | **Current** |
//!
//! [`ThemeTokens`] deserializes both formats automatically.  Legacy flat-map JSON
//! is detected by the absence of any structured key (`"colors"`, `"typography"`,
//! etc.) and migrated to the structured format using the built-in defaults for
//! any absent fields.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

// ── Full ThemeTokens (current format) ─────────────────────────────────────────

/// Full Zoid design token set.
///
/// Covers color roles, typography, spacing, corner radii, shadows, and motion.
/// Every color and shadow group carries explicit `light` and `dark` variant
/// fields; all other token groups are mode-independent.
///
/// # Serde notes
///
/// Both the current structured format and the deprecated 1.3.3 flat-map format
/// are accepted on deserialization.  The flat-map format is automatically
/// migrated; any unrecognised legacy keys are silently ignored.  Serialization
/// always writes the current structured format.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ThemeTokens {
    /// Color role palette for light and dark surfaces.
    pub colors: ColorTokens,
    /// Typography scale (font families, sizes, weights, line heights).
    pub typography: TypographyTokens,
    /// Spacing scale in logical pixels.
    pub spacing: SpacingTokens,
    /// Corner radius scale in logical pixels.
    pub radius: RadiusTokens,
    /// Box-shadow definitions for light and dark surfaces.
    pub shadow: ShadowTokens,
    /// Animation timing tokens.
    pub motion: MotionTokens,
}

impl<'de> Deserialize<'de> for ThemeTokens {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = ThemeRaw::deserialize(deserializer)?;
        Ok(ThemeTokens::from(raw))
    }
}

impl ThemeTokens {
    /// Returns the color tokens for the requested appearance mode.
    ///
    /// `dark == true` returns the dark-mode palette; otherwise the light
    /// palette is returned.
    #[must_use]
    pub fn colors_for(&self, dark: bool) -> &ColorPalette {
        if dark {
            &self.colors.dark
        } else {
            &self.colors.light
        }
    }

    /// Returns the shadow tokens for the requested appearance mode.
    #[must_use]
    pub fn shadow_for(&self, dark: bool) -> &ShadowSet {
        if dark {
            &self.shadow.dark
        } else {
            &self.shadow.light
        }
    }

    /// Migrates a raw flat-map (legacy 1.3.3 format) to a full [`ThemeTokens`].
    ///
    /// Recognised flat-map keys are mapped to their corresponding typed fields;
    /// all unrecognised keys are silently ignored.  Absent fields receive
    /// built-in defaults.
    #[must_use]
    pub fn migrate_from_flat(flat: &HashMap<String, JsonValue>) -> Self {
        let mut tokens = ThemeTokens::default();

        // ── Colors (applied to the light variant only; dark keeps its default) ──
        if let Some(v) = flat_str(flat, "color.primary") {
            tokens.colors.light.primary = v;
        }
        if let Some(v) = flat_str(flat, "color.primary-hover") {
            tokens.colors.light.primary_hover = v;
        }
        if let Some(v) = flat_str(flat, "color.secondary") {
            tokens.colors.light.secondary = v;
        }
        if let Some(v) = flat_str(flat, "color.secondary-hover") {
            tokens.colors.light.secondary_hover = v;
        }
        if let Some(v) = flat_str(flat, "color.background") {
            tokens.colors.light.background = v;
        }
        if let Some(v) = flat_str(flat, "color.surface") {
            tokens.colors.light.surface = v;
        }
        if let Some(v) = flat_str(flat, "color.surface-hover") {
            tokens.colors.light.surface_hover = v;
        }
        if let Some(v) = flat_str(flat, "color.surface-elevated") {
            tokens.colors.light.surface_elevated = v;
        }
        if let Some(v) = flat_str(flat, "color.border") {
            tokens.colors.light.border = v;
        }
        if let Some(v) = flat_str(flat, "color.text") {
            tokens.colors.light.text = v;
        }
        if let Some(v) = flat_str(flat, "color.text-muted") {
            tokens.colors.light.text_muted = v;
        }
        if let Some(v) = flat_str(flat, "color.accent") {
            tokens.colors.light.accent = v;
        }
        if let Some(v) = flat_str(flat, "color.accent-hover") {
            tokens.colors.light.accent_hover = v;
        }
        if let Some(v) = flat_str(flat, "color.accent-text") {
            tokens.colors.light.accent_text = v;
        }
        if let Some(v) = flat_str(flat, "color.danger") {
            tokens.colors.light.danger = v;
        }
        if let Some(v) = flat_str(flat, "color.warning") {
            tokens.colors.light.warning = v;
        }
        if let Some(v) = flat_str(flat, "color.success") {
            tokens.colors.light.success = v;
        }
        if let Some(v) = flat_str(flat, "color.info") {
            tokens.colors.light.info = v;
        }

        // ── Typography ────────────────────────────────────────────────────────
        if let Some(v) = flat_str(flat, "font.family.sans") {
            tokens.typography.font_family_sans = v;
        }
        if let Some(v) = flat_str(flat, "font.family.mono") {
            tokens.typography.font_family_mono = v;
        }
        if let Some(v) = flat_f32(flat, "font.size.xs") {
            tokens.typography.size_xs = v;
        }
        if let Some(v) = flat_f32(flat, "font.size.sm") {
            tokens.typography.size_sm = v;
        }
        // "font.size.base" and "font.size.md" both map to size_md.
        if let Some(v) = flat_f32(flat, "font.size.base") {
            tokens.typography.size_md = v;
        }
        if let Some(v) = flat_f32(flat, "font.size.md") {
            tokens.typography.size_md = v;
        }
        if let Some(v) = flat_f32(flat, "font.size.lg") {
            tokens.typography.size_lg = v;
        }
        if let Some(v) = flat_f32(flat, "font.size.xl") {
            tokens.typography.size_xl = v;
        }
        if let Some(v) = flat_f32(flat, "font.size.2xl") {
            tokens.typography.size_2xl = v;
        }
        if let Some(v) = flat_f32(flat, "font.line-height.tight") {
            tokens.typography.line_height_tight = v;
        }
        if let Some(v) = flat_f32(flat, "font.line-height.normal") {
            tokens.typography.line_height_normal = v;
        }

        // ── Spacing ───────────────────────────────────────────────────────────
        if let Some(v) = flat_f32(flat, "spacing.xs") {
            tokens.spacing.xs = v;
        }
        if let Some(v) = flat_f32(flat, "spacing.sm") {
            tokens.spacing.sm = v;
        }
        if let Some(v) = flat_f32(flat, "spacing.md") {
            tokens.spacing.md = v;
        }
        if let Some(v) = flat_f32(flat, "spacing.lg") {
            tokens.spacing.lg = v;
        }
        if let Some(v) = flat_f32(flat, "spacing.xl") {
            tokens.spacing.xl = v;
        }
        if let Some(v) = flat_f32(flat, "spacing.2xl") {
            tokens.spacing.xxl = v;
        }

        // ── Radius ────────────────────────────────────────────────────────────
        if let Some(v) = flat_f32(flat, "radius.sm") {
            tokens.radius.sm = v;
        }
        if let Some(v) = flat_f32(flat, "radius.md") {
            tokens.radius.md = v;
        }
        if let Some(v) = flat_f32(flat, "radius.lg") {
            tokens.radius.lg = v;
        }
        if let Some(v) = flat_f32(flat, "radius.full") {
            tokens.radius.full = v;
        }

        tokens
    }
}

/// Private intermediate used only during deserialization.
///
/// Named fields capture the current structured format; `rest` catches any
/// remaining JSON keys (including legacy flat-map dotted keys).
#[derive(Deserialize)]
struct ThemeRaw {
    #[serde(default)]
    colors: Option<ColorTokens>,
    #[serde(default)]
    typography: Option<TypographyTokens>,
    #[serde(default)]
    spacing: Option<SpacingTokens>,
    #[serde(default)]
    radius: Option<RadiusTokens>,
    #[serde(default)]
    shadow: Option<ShadowTokens>,
    #[serde(default)]
    motion: Option<MotionTokens>,
    /// Any keys not matched by the named fields above (legacy flat-map entries
    /// or future unknown keys) land here.
    #[serde(flatten)]
    rest: HashMap<String, JsonValue>,
}

impl From<ThemeRaw> for ThemeTokens {
    fn from(raw: ThemeRaw) -> Self {
        let has_new_format = raw.colors.is_some()
            || raw.typography.is_some()
            || raw.spacing.is_some()
            || raw.radius.is_some()
            || raw.shadow.is_some()
            || raw.motion.is_some();

        if has_new_format {
            ThemeTokens {
                colors: raw.colors.unwrap_or_default(),
                typography: raw.typography.unwrap_or_default(),
                spacing: raw.spacing.unwrap_or_default(),
                radius: raw.radius.unwrap_or_default(),
                shadow: raw.shadow.unwrap_or_default(),
                motion: raw.motion.unwrap_or_default(),
            }
        } else if raw.rest.is_empty() {
            ThemeTokens::default()
        } else {
            ThemeTokens::migrate_from_flat(&raw.rest)
        }
    }
}

// ── Deprecated: legacy 1.3.3 flat-map type ────────────────────────────────────

/// Deprecated flat-map theme token format from schema 1.3.3.
///
/// This type is preserved for documentation and migration purposes only.
/// All new code must use [`ThemeTokens`] with its structured token groups.
///
/// When loading a `zoid-project.json` that contains the legacy flat-map format,
/// [`ThemeTokens`] deserializes it automatically and migrates the recognized
/// keys to their corresponding structured fields.
///
/// # Example (legacy JSON)
///
/// ```json
/// "theme": {
///   "color.primary": "#3c82f6",
///   "font.size.base": "16px"
/// }
/// ```
#[deprecated(
    since = "1.9.1",
    note = "Use `ThemeTokens` with structured token groups instead. \
            Legacy flat-map JSON is automatically migrated on deserialization."
)]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LegacyThemeTokens {
    /// Flat map of token name → string value.
    ///
    /// Keys use dot-separated paths such as `"color.primary"`,
    /// `"font.size.base"`, etc.
    #[serde(default, flatten)]
    pub tokens: HashMap<String, String>,
}

// ── Color tokens ──────────────────────────────────────────────────────────────

/// Color role palette for light and dark application surfaces.
///
/// Both variants must be present; they share the same role names but carry
/// appropriately adjusted hex values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorTokens {
    /// Light-mode color palette.
    #[serde(default = "ColorPalette::light")]
    pub light: ColorPalette,
    /// Dark-mode color palette.
    #[serde(default = "ColorPalette::dark")]
    pub dark: ColorPalette,
}

impl Default for ColorTokens {
    fn default() -> Self {
        Self {
            light: ColorPalette::light(),
            dark: ColorPalette::dark(),
        }
    }
}

/// A complete set of semantic color roles for one appearance mode.
///
/// All values are CSS hex color strings (e.g. `"#3c82f6"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorPalette {
    /// Primary brand / action color.
    pub primary: String,
    /// Hovered primary color.
    pub primary_hover: String,
    /// Secondary / subdued color.
    pub secondary: String,
    /// Hovered secondary color.
    pub secondary_hover: String,
    /// Window or page background.
    pub background: String,
    /// Card or panel surface.
    pub surface: String,
    /// Hovered surface.
    pub surface_hover: String,
    /// Elevated (floating) surface (dialogs, popovers).
    pub surface_elevated: String,
    /// Border and divider color.
    pub border: String,
    /// Primary body text.
    pub text: String,
    /// Muted / secondary text.
    pub text_muted: String,
    /// Interactive accent (links, highlights).
    pub accent: String,
    /// Hovered accent.
    pub accent_hover: String,
    /// Text rendered on top of an accent-colored surface.
    pub accent_text: String,
    /// Destructive / error status color.
    pub danger: String,
    /// Warning status color.
    pub warning: String,
    /// Success / positive status color.
    pub success: String,
    /// Informational status color.
    pub info: String,
}

impl ColorPalette {
    /// Returns the built-in light-mode color palette.
    #[must_use]
    pub fn light() -> Self {
        Self {
            primary: "#2563eb".to_owned(),
            primary_hover: "#1d4ed8".to_owned(),
            secondary: "#606a78".to_owned(),
            secondary_hover: "#4b5563".to_owned(),
            background: "#f6f7f9".to_owned(),
            surface: "#ffffff".to_owned(),
            surface_hover: "#ebeef2".to_owned(),
            surface_elevated: "#ffffff".to_owned(),
            border: "#cbd3df".to_owned(),
            text: "#1f2328".to_owned(),
            text_muted: "#606a78".to_owned(),
            accent: "#2563eb".to_owned(),
            accent_hover: "#1d4ed8".to_owned(),
            accent_text: "#ffffff".to_owned(),
            danger: "#b42318".to_owned(),
            warning: "#9a6700".to_owned(),
            success: "#1f7a4d".to_owned(),
            info: "#0369a1".to_owned(),
        }
    }

    /// Returns the built-in dark-mode color palette.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            primary: "#7aa2ff".to_owned(),
            primary_hover: "#96b5ff".to_owned(),
            secondary: "#b6bfcc".to_owned(),
            secondary_hover: "#cbd5e1".to_owned(),
            background: "#202124".to_owned(),
            surface: "#2b2d31".to_owned(),
            surface_hover: "#363a42".to_owned(),
            surface_elevated: "#30333a".to_owned(),
            border: "#464c56".to_owned(),
            text: "#f5f7fa".to_owned(),
            text_muted: "#b6bfcc".to_owned(),
            accent: "#7aa2ff".to_owned(),
            accent_hover: "#96b5ff".to_owned(),
            accent_text: "#101216".to_owned(),
            danger: "#ff8a7a".to_owned(),
            warning: "#f2c94c".to_owned(),
            success: "#72d6a4".to_owned(),
            info: "#38bdf8".to_owned(),
        }
    }
}

impl Default for ColorPalette {
    /// The default `ColorPalette` is the light variant.
    fn default() -> Self {
        Self::light()
    }
}

// ── Typography tokens ─────────────────────────────────────────────────────────

/// Typography scale — mode-independent font settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyTokens {
    /// Primary sans-serif font family stack.
    pub font_family_sans: String,
    /// Monospace font family stack (code, logs, etc.).
    pub font_family_mono: String,
    /// Extra-small text size in logical pixels.
    pub size_xs: f32,
    /// Small text size in logical pixels.
    pub size_sm: f32,
    /// Body / base text size in logical pixels.
    pub size_md: f32,
    /// Large text size in logical pixels.
    pub size_lg: f32,
    /// Extra-large text size in logical pixels.
    pub size_xl: f32,
    /// 2× extra-large text size in logical pixels.
    pub size_2xl: f32,
    /// Tight line-height multiplier (compact labels, headings).
    pub line_height_tight: f32,
    /// Normal line-height multiplier (body text).
    pub line_height_normal: f32,
    /// Normal font weight.
    pub font_weight_normal: u16,
    /// Medium font weight.
    pub font_weight_medium: u16,
    /// Bold font weight.
    pub font_weight_bold: u16,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        Self {
            font_family_sans: "system-ui, -apple-system, sans-serif".to_owned(),
            font_family_mono: "ui-monospace, 'Cascadia Code', monospace".to_owned(),
            size_xs: 11.0,
            size_sm: 12.0,
            size_md: 14.0,
            size_lg: 16.0,
            size_xl: 20.0,
            size_2xl: 24.0,
            line_height_tight: 1.2,
            line_height_normal: 1.45,
            font_weight_normal: 400,
            font_weight_medium: 500,
            font_weight_bold: 700,
        }
    }
}

// ── Spacing tokens ────────────────────────────────────────────────────────────

/// Spacing scale — mode-independent size steps in logical pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpacingTokens {
    /// 4 px — tight padding or icon gap.
    pub xs: f32,
    /// 8 px — small internal padding.
    pub sm: f32,
    /// 12 px — standard internal padding.
    pub md: f32,
    /// 16 px — comfortable section gap.
    pub lg: f32,
    /// 24 px — large section gap.
    pub xl: f32,
    /// 32 px — extra-large gap or page margin.
    pub xxl: f32,
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
            xxl: 32.0,
        }
    }
}

// ── Radius tokens ─────────────────────────────────────────────────────────────

/// Corner radius scale — mode-independent in logical pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadiusTokens {
    /// Small radius (inputs, chips).
    pub sm: f32,
    /// Medium radius (buttons, cards).
    pub md: f32,
    /// Large radius (panels, dialogs).
    pub lg: f32,
    /// Full radius (pill shapes, avatars).
    pub full: f32,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            sm: 4.0,
            md: 6.0,
            lg: 8.0,
            full: 9999.0,
        }
    }
}

// ── Shadow tokens ─────────────────────────────────────────────────────────────

/// Box-shadow definitions for light and dark surfaces.
///
/// Values are CSS `box-shadow` strings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowTokens {
    /// Light-mode shadow set.
    #[serde(default = "ShadowSet::light")]
    pub light: ShadowSet,
    /// Dark-mode shadow set.
    #[serde(default = "ShadowSet::dark")]
    pub dark: ShadowSet,
}

impl Default for ShadowTokens {
    fn default() -> Self {
        Self {
            light: ShadowSet::light(),
            dark: ShadowSet::dark(),
        }
    }
}

/// A set of three shadow definitions (sm / md / lg) for one appearance mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShadowSet {
    /// Small elevation shadow.
    pub sm: String,
    /// Medium elevation shadow.
    pub md: String,
    /// Large elevation shadow.
    pub lg: String,
}

impl ShadowSet {
    /// Returns the built-in light-mode shadows.
    #[must_use]
    pub fn light() -> Self {
        Self {
            sm: "0 1px 3px rgba(0,0,0,0.12)".to_owned(),
            md: "0 4px 6px rgba(0,0,0,0.10)".to_owned(),
            lg: "0 10px 15px rgba(0,0,0,0.10)".to_owned(),
        }
    }

    /// Returns the built-in dark-mode shadows.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            sm: "0 1px 3px rgba(0,0,0,0.40)".to_owned(),
            md: "0 4px 6px rgba(0,0,0,0.35)".to_owned(),
            lg: "0 10px 15px rgba(0,0,0,0.35)".to_owned(),
        }
    }
}

impl Default for ShadowSet {
    fn default() -> Self {
        Self::light()
    }
}

// ── Motion tokens ─────────────────────────────────────────────────────────────

/// Animation timing tokens — mode-independent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionTokens {
    /// Fast transition duration in milliseconds (micro-interactions).
    pub duration_fast_ms: u32,
    /// Normal transition duration in milliseconds.
    pub duration_normal_ms: u32,
    /// Slow transition duration in milliseconds (large layout shifts).
    pub duration_slow_ms: u32,
    /// Standard CSS easing function (most transitions).
    pub easing_standard: String,
    /// Deceleration easing — elements entering the screen.
    pub easing_decelerate: String,
    /// Acceleration easing — elements leaving the screen.
    pub easing_accelerate: String,
}

impl Default for MotionTokens {
    fn default() -> Self {
        Self {
            duration_fast_ms: 100,
            duration_normal_ms: 200,
            duration_slow_ms: 300,
            easing_standard: "cubic-bezier(0.4, 0, 0.2, 1)".to_owned(),
            easing_decelerate: "cubic-bezier(0, 0, 0.2, 1)".to_owned(),
            easing_accelerate: "cubic-bezier(0.4, 0, 1, 1)".to_owned(),
        }
    }
}

// ── Internal flat-map helpers ─────────────────────────────────────────────────

/// Returns the string value of a flat-map entry, if it is a JSON string.
fn flat_str(map: &HashMap<String, JsonValue>, key: &str) -> Option<String> {
    map.get(key)?.as_str().map(str::to_owned)
}

/// Returns the numeric value of a flat-map entry.
///
/// Accepts both JSON numbers and CSS pixel strings like `"16px"`.
fn flat_f32(map: &HashMap<String, JsonValue>, key: &str) -> Option<f32> {
    let v = map.get(key)?;
    if let Some(n) = v.as_f64() {
        return Some(n as f32);
    }
    if let Some(s) = v.as_str() {
        let trimmed = s.trim().trim_end_matches("px").trim();
        return trimmed.parse::<f32>().ok();
    }
    None
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_tokens_round_trips() {
        let original = ThemeTokens::default();
        let json = serde_json::to_string(&original).expect("serialize");
        let loaded: ThemeTokens = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, loaded);
    }

    #[test]
    fn legacy_flat_map_deserializes_via_migration() {
        let json = r##"{"color.primary":"#3c82f6","font.size.base":"16px"}"##;
        let tokens: ThemeTokens = serde_json::from_str(json).expect("deserialize legacy");
        assert_eq!(tokens.colors.light.primary, "#3c82f6");
        assert_eq!(tokens.typography.size_md, 16.0);
        // Dark palette remains at its built-in default.
        assert_eq!(tokens.colors.dark, ColorPalette::dark());
    }

    #[test]
    fn empty_object_deserializes_to_default() {
        let tokens: ThemeTokens = serde_json::from_str("{}").expect("deserialize empty");
        assert_eq!(tokens, ThemeTokens::default());
    }

    #[test]
    fn flat_f32_parses_px_string() {
        let mut map = HashMap::new();
        map.insert("k".to_owned(), JsonValue::String("16px".to_owned()));
        assert_eq!(flat_f32(&map, "k"), Some(16.0));
    }

    #[test]
    fn flat_f32_parses_number() {
        let mut map = HashMap::new();
        map.insert(
            "k".to_owned(),
            JsonValue::Number(serde_json::Number::from_f64(14.0).unwrap()),
        );
        assert_eq!(flat_f32(&map, "k"), Some(14.0));
    }

    #[test]
    fn colors_for_returns_correct_variant() {
        let tokens = ThemeTokens::default();
        assert_eq!(tokens.colors_for(false), &ColorPalette::light());
        assert_eq!(tokens.colors_for(true), &ColorPalette::dark());
    }
}
