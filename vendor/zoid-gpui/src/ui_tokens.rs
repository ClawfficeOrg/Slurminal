//! Layout token scales for handrolled `zoid_gpui` widgets.
//!
//! These are the non-color design tokens (spacing, radius, typography, focus
//! ring) that survived the v8 move to `gpui_kit::component::Theme`.  Colors now come
//! from the active `Theme`; these scales remain because gpui-component does not
//! expose equivalents the KEEP-custom widgets can read.

use gpui::{Hsla, Rgba, rgb};

/// Feature id under which the UI-kit token defaults are registered.
pub const UI_KIT_TOKENS_FEATURE_ID: &str = "ui-kit-tokens";

/// Spacing scale in logical pixels (padding, gaps, insets).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpacingScale {
    /// Extra-small spacing.
    pub xs: f32,
    /// Small spacing.
    pub sm: f32,
    /// Medium spacing.
    pub md: f32,
    /// Large spacing.
    pub lg: f32,
    /// Extra-large spacing.
    pub xl: f32,
}

impl Default for SpacingScale {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 12.0,
            lg: 16.0,
            xl: 24.0,
        }
    }
}

/// Corner-radius scale in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadiusScale {
    /// Small radius.
    pub sm: f32,
    /// Medium radius.
    pub md: f32,
    /// Large radius.
    pub lg: f32,
}

impl Default for RadiusScale {
    fn default() -> Self {
        Self {
            sm: 4.0,
            md: 6.0,
            lg: 8.0,
        }
    }
}

/// Typography scale: font sizes (logical px) and unitless line-height ratios.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TypographyScale {
    /// Small font size.
    pub sm: f32,
    /// Medium (body) font size.
    pub md: f32,
    /// Large font size.
    pub lg: f32,
    /// Tight line-height ratio (headings, pills).
    pub line_height_tight: f32,
    /// Default line-height ratio (body text).
    pub line_height: f32,
}

impl Default for TypographyScale {
    fn default() -> Self {
        Self {
            sm: 12.0,
            md: 14.0,
            lg: 16.0,
            line_height_tight: 1.2,
            line_height: 1.45,
        }
    }
}

/// Focus-ring appearance (color + geometry) for focusable widgets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusRingMetrics {
    /// Ring color.
    pub color: Hsla,
    /// Ring stroke width in logical pixels.
    pub width: f32,
    /// Gap between the widget edge and the ring in logical pixels.
    pub offset: f32,
}

impl FocusRingMetrics {
    /// Focus-ring metrics tuned for light backgrounds.
    pub fn light() -> Self {
        Self {
            color: color(0x2563eb),
            width: 2.0,
            offset: 2.0,
        }
    }

    /// Focus-ring metrics tuned for dark backgrounds.
    pub fn dark() -> Self {
        Self {
            color: color(0x96b5ff),
            width: 2.0,
            offset: 2.0,
        }
    }
}

impl Default for FocusRingMetrics {
    fn default() -> Self {
        Self::light()
    }
}

fn color(hex: u32) -> Hsla {
    Hsla::from(rgb(hex))
}

/// Converts an [`Hsla`] color to a packed `0xRRGGBB` integer (alpha dropped).
#[must_use]
pub fn color_to_hex(color: Hsla) -> u32 {
    let rgba = Rgba::from(color);
    let r = (rgba.r * 255.0).round() as u32;
    let g = (rgba.g * 255.0).round() as u32;
    let b = (rgba.b * 255.0).round() as u32;
    (r << 16) | (g << 8) | b
}
