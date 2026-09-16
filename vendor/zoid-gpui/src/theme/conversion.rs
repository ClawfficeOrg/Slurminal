//! Conversion helpers: [`ThemeTokens`] → [`gpui_kit::component::ThemeColor`].
//!
//! [`ThemeTokens`] stores colors as CSS hex strings (e.g. `"#3c82f6"`).
//! This module parses those strings into [`Hsla`] values and constructs
//! a fully populated [`gpui_kit::component::ThemeColor`] for either light or dark
//! mode.
//!
//! # Example
//!
//! ```no_run
//! use zoid_core::schema::ThemeTokens;
//! use zoid_gpui::theme::theme_tokens_to_theme_color;
//!
//! let tokens = ThemeTokens::default();
//! let theme_color = theme_tokens_to_theme_color(&tokens, /* dark = */ true);
//! ```

use gpui::{Hsla, Rgba, rgb};
use gpui_kit::component::ThemeColor;

use zoid_core::schema::ThemeTokens;

/// Converts a [`ThemeTokens`] value to a [`gpui_kit::component::ThemeColor`].
///
/// When `dark` is `true` the dark color palette is used; otherwise the light
/// palette is used.
#[must_use]
pub fn theme_tokens_to_theme_color(tokens: &ThemeTokens, dark: bool) -> ThemeColor {
    let palette = tokens.colors_for(dark);

    ThemeColor {
        background: parse_hex_or_default(&palette.background, 0x202124),
        secondary: parse_hex_or_default(&palette.surface, 0x2b2d31),
        border: parse_hex_or_default(&palette.border, 0x464c56),
        foreground: parse_hex_or_default(&palette.text, 0xf5f7fa),
        muted_foreground: parse_hex_or_default(&palette.text_muted, 0xb6bfcc),
        accent: parse_hex_or_default(&palette.accent, 0x7aa2ff),
        accent_foreground: parse_hex_or_default(&palette.accent_text, 0x101216),
        danger: parse_hex_or_default(&palette.danger, 0xff8a7a),
        warning: parse_hex_or_default(&palette.warning, 0xf2c94c),
        success: parse_hex_or_default(&palette.success, 0x72d6a4),
        ..Default::default()
    }
}

/// Parses a CSS hex color string (e.g. `"#3c82f6"`) to [`Hsla`].
///
/// Returns the color specified by `fallback_hex` if the string cannot be
/// parsed.  This ensures we never panic on malformed user-supplied values.
fn parse_hex_or_default(hex: &str, fallback_hex: u32) -> Hsla {
    parse_hex(hex).unwrap_or_else(|| Hsla::from(rgb(fallback_hex)))
}

/// Parses a CSS hex color string to [`Hsla`].
///
/// Accepts 6-digit hex strings with an optional leading `#`.
/// Returns `None` if the string does not match the expected format.
fn parse_hex(hex: &str) -> Option<Hsla> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let rgba = Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    };
    Some(Hsla::from(rgba))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::ui_tokens::{RadiusScale, SpacingScale, TypographyScale, color_to_hex};

    #[test]
    fn parse_hex_valid_6_digit() {
        let h = parse_hex("#3c82f6").expect("valid hex");
        assert_eq!(color_to_hex(h), 0x3c82f6);
    }

    #[test]
    fn parse_hex_without_hash() {
        let h = parse_hex("ffffff").expect("valid hex without #");
        assert_eq!(color_to_hex(h), 0xffffff);
    }

    #[test]
    fn parse_hex_invalid_returns_none() {
        assert!(parse_hex("gg0000").is_none());
        assert!(parse_hex("#abc").is_none());
        assert!(parse_hex("").is_none());
    }

    #[test]
    fn parse_hex_or_default_falls_back_on_bad_input() {
        let fallback = parse_hex_or_default("not-a-color", 0xaabbcc);
        assert_eq!(color_to_hex(fallback), 0xaabbcc);
    }

    #[test]
    fn default_tokens_light_mode_matches_theme_color_light() {
        let tokens = ThemeTokens::default();
        let theme_color = theme_tokens_to_theme_color(&tokens, false);
        assert_eq!(color_to_hex(theme_color.background), 0xf6f7f9);
    }

    #[test]
    fn default_tokens_dark_mode_matches_theme_color_dark() {
        let tokens = ThemeTokens::default();
        let theme_color = theme_tokens_to_theme_color(&tokens, true);
        assert_eq!(color_to_hex(theme_color.background), 0x202124);
    }

    #[test]
    fn conversion_preserves_spacing_and_radius() {
        let tokens = ThemeTokens::default();
        let _theme_color = theme_tokens_to_theme_color(&tokens, false);
        assert_eq!(SpacingScale::default().sm, tokens.spacing.sm);
        assert_eq!(SpacingScale::default().md, tokens.spacing.md);
        assert_eq!(RadiusScale::default().lg, tokens.radius.lg);
    }

    #[test]
    fn conversion_preserves_typography() {
        let tokens = ThemeTokens::default();
        let _theme_color = theme_tokens_to_theme_color(&tokens, false);
        assert_eq!(TypographyScale::default().md, tokens.typography.size_md);
        assert_eq!(
            TypographyScale::default().line_height,
            tokens.typography.line_height_normal
        );
    }

    #[test]
    fn custom_color_roundtrips_through_conversion() {
        use zoid_core::schema::{ColorPalette, ColorTokens};

        let mut tokens = ThemeTokens::default();
        tokens.colors.light = ColorPalette {
            background: "#ff0000".to_owned(),
            ..ColorPalette::light()
        };
        tokens.colors.dark = ColorTokens::default().dark;

        let theme_color = theme_tokens_to_theme_color(&tokens, false);
        assert_eq!(color_to_hex(theme_color.background), 0xff0000);
    }
}
