//! GPUI terminal surface: grid render surface (task 9.2.1).
//!
//! The surface reads [`crate::terminal::OwnedSnapshot`] — one owned frame
//! captured from the render-state API — and turns it into a GPUI element.
//! The whole frame resolves from a single snapshot, so a rendered frame is
//! always atomic: no flicker from half-applied output, by construction.
//!
//! [`GridFrame`] (in [`grid`]) carries the snapshot's [`Dirty`] state plus
//! per-row slots, so a damaged-region redraw can later skip clean rows
//! without reshaping any of this.

pub mod fonts;
pub mod grid;
pub mod input;
pub mod keys;
pub mod search;
pub mod selection;
pub mod theme;
pub mod view;

pub use fonts::{
    FontSelection, NERD_FONT_CANDIDATES, PrimarySource, is_nerd_font_family, terminal_font,
};
pub use grid::{GridFrame, ResolvedCell, ResolvedRow, TerminalGrid};
pub use keys::key_press_from_keystroke;
pub use search::{SearchMatch, SearchOptions, SearchState, find_matches, locate};
pub use selection::{CellPoint, ScrollIndicator, SelectionState, scroll_lines, snap_to_live};
pub use theme::{BRIGHT_WHITE_LIFT, PaletteSource, TerminalPalette, hsla_to_rgb, lighten};
pub use view::{TerminalEvent, TerminalView};

use gpui::{App, Pixels, SharedString, Size, px};

/// Monospace face used when the app does not name one.
///
/// GPUI's Windows DirectWrite backend does **not** resolve the CSS generic
/// `monospace` keyword — it looks up a literal face named "monospace",
/// finds none, and falls back unpredictably. Name a face the target OS
/// actually ships (same rule as `zoid_gpui::MONO_FONT_FAMILY`). Nerd Font
/// detection and per-glyph fallback are task 9.2.2's job, not this one's.
pub const DEFAULT_MONO_FAMILY: &str = if cfg!(target_os = "windows") {
    "Consolas"
} else if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "monospace"
};

/// Default terminal font size in pixels.
pub const DEFAULT_FONT_SIZE_PX: f32 = 14.0;

/// Surface configuration: the things ZML can set plus what code supplies.
///
/// Matches the ZML-side props in `docs/terminal-component.md`
/// (`font_family`, `font_size`, `palette`); scrollback/cwd arrive with 9.3.1.
#[derive(Clone, Debug)]
pub struct SurfaceConfig {
    /// Font family for every cell. Defaults to [`DEFAULT_MONO_FAMILY`].
    pub font_family: SharedString,
    /// Font size in pixels. Cell metrics derive from this.
    pub font_size: Pixels,
    /// Where surface colors come from: the app theme or a standalone
    /// palette. Defaults to [`PaletteSource::Theme`].
    pub palette: PaletteSource,
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self {
            font_family: DEFAULT_MONO_FAMILY.into(),
            font_size: px(DEFAULT_FONT_SIZE_PX),
            palette: PaletteSource::Theme,
        }
    }
}

/// Measured cell box in pixels, resolved from the real font.
///
/// Width is the advance of `m` (`TextSystem::em_advance`); the surface font
/// is monospace by contract, so every glyph shares that advance. Height is
/// `ascent + |descent|`: GPUI's `TextSystem::descent` follows the font
/// tables' sign convention (negative, below the baseline) on every
/// platform, so a plain sum would shrink rows by twice the descent and
/// overlap them (found by the 9.3.5 Slurminal embedding). Rows stack at
/// exactly this height, so the grid aligns by construction — nothing here
/// is a hardcoded constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellMetrics {
    /// Horizontal advance of one cell.
    pub cell_width: Pixels,
    /// Vertical advance of one row.
    pub cell_height: Pixels,
}

impl CellMetrics {
    /// Fallback box when the font cannot be measured at all.
    ///
    /// Only used when `em_advance` errors (font missing from the system).
    /// Ratios, not pixel constants, so the box still tracks `font_size`.
    fn fallback(font_size: Pixels) -> Self {
        Self {
            cell_width: font_size * 0.6,
            cell_height: font_size * 1.2,
        }
    }
}

/// Measure [`CellMetrics`] from the resolved font.
///
/// Must run with an `App` context (`#[gpui::test]` or a live view): the
/// measurement goes through `cx.text_system()`, the same shaper the
/// renderer uses, so measured and drawn advances agree.
#[must_use]
pub fn measure_cell_metrics(cx: &App, config: &SurfaceConfig) -> CellMetrics {
    // Measure through the same selection the element renders with, so
    // fallbacks are in play for both: if the primary is missing, the
    // resolved face (not a guess) supplies the advance.
    let font = terminal_font(config).font;
    let system = cx.text_system();
    let font_id = system.resolve_font(&font);
    let width = system
        .em_advance(font_id, config.font_size)
        .unwrap_or_else(|_| CellMetrics::fallback(config.font_size).cell_width);
    let ascent = system.ascent(font_id, config.font_size);
    let descent = system.descent(font_id, config.font_size);
    let height = ascent + px(f32::from(descent).abs());
    CellMetrics {
        cell_width: width,
        cell_height: height,
    }
}

/// Grid dimensions for a viewport of `viewport` pixels.
///
/// Floors each axis and clamps to at least 1x1 so a zero-size layout pass
/// still yields a usable terminal. The app forwards a change here to both
/// `Terminal::resize` and the PTY (`PtySession::resize`) per the component
/// contract; compare with [`needs_resize`] first to avoid resize storms.
#[must_use]
pub fn grid_dims_for_viewport(viewport: Size<Pixels>, metrics: &CellMetrics) -> (u16, u16) {
    let cols = (f32::from(viewport.width) / f32::from(metrics.cell_width)).floor() as u32;
    let rows = (f32::from(viewport.height) / f32::from(metrics.cell_height)).floor() as u32;
    (
        cols.clamp(1, u16::MAX as u32) as u16,
        rows.clamp(1, u16::MAX as u32) as u16,
    )
}

/// Whether the viewport-derived dims differ from the live terminal dims.
#[must_use]
pub const fn needs_resize(current: (u16, u16), next: (u16, u16)) -> bool {
    current.0 != next.0 || current.1 != next.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_math_floors_each_axis() {
        let metrics = CellMetrics {
            cell_width: px(8.0),
            cell_height: px(16.0),
        };
        assert_eq!(
            grid_dims_for_viewport(
                Size {
                    width: px(800.0),
                    height: px(600.0),
                },
                &metrics
            ),
            (100, 37)
        );
    }

    #[test]
    fn viewport_math_clamps_to_at_least_one() {
        let metrics = CellMetrics {
            cell_width: px(8.0),
            cell_height: px(16.0),
        };
        assert_eq!(
            grid_dims_for_viewport(
                Size {
                    width: px(0.0),
                    height: px(0.0),
                },
                &metrics
            ),
            (1, 1)
        );
        assert_eq!(
            grid_dims_for_viewport(
                Size {
                    width: px(4.0),
                    height: px(15.9),
                },
                &metrics
            ),
            (1, 1)
        );
    }

    #[test]
    fn needs_resize_compares_both_axes() {
        assert!(!needs_resize((80, 24), (80, 24)));
        assert!(needs_resize((80, 24), (81, 24)));
        assert!(needs_resize((80, 24), (80, 25)));
    }

    #[test]
    fn fallback_metrics_track_font_size() {
        let small = CellMetrics::fallback(px(14.0));
        let large = CellMetrics::fallback(px(28.0));
        assert_eq!(
            f32::from(small.cell_width) * 2.0,
            f32::from(large.cell_width)
        );
        assert_eq!(
            f32::from(small.cell_height) * 2.0,
            f32::from(large.cell_height)
        );
    }

    /// Regression (9.3.5): GPUI reports descent as a negative offset, so
    /// `ascent + descent` made rows shorter than the glyphs' ascent alone
    /// and the grid painted each row over the one above.
    #[gpui::test]
    fn cell_height_spans_ascent_and_descent(cx: &mut gpui::TestAppContext) {
        let config = SurfaceConfig::default();
        let (metrics, ascent, descent) = cx.update(|cx| {
            let system = cx.text_system();
            let font_id = system.resolve_font(&terminal_font(&config).font);
            (
                measure_cell_metrics(cx, &config),
                system.ascent(font_id, config.font_size),
                system.descent(font_id, config.font_size),
            )
        });
        let expected = f32::from(ascent) + f32::from(descent).abs();
        assert!((f32::from(metrics.cell_height) - expected).abs() < 0.01);
        assert!(metrics.cell_height >= ascent);
    }

    #[gpui::test]
    fn cell_metrics_come_from_the_font(cx: &mut gpui::TestAppContext) {
        let small = SurfaceConfig {
            font_family: DEFAULT_MONO_FAMILY.into(),
            font_size: px(14.0),
            palette: PaletteSource::Theme,
        };
        let large = SurfaceConfig {
            font_family: DEFAULT_MONO_FAMILY.into(),
            font_size: px(28.0),
            palette: PaletteSource::Theme,
        };
        let (small_m, large_m) = cx.update(|cx| {
            (
                measure_cell_metrics(cx, &small),
                measure_cell_metrics(cx, &large),
            )
        });
        assert!(f32::from(small_m.cell_width) > 0.0);
        assert!(f32::from(small_m.cell_height) > 0.0);
        // Font-derived, not hardcoded: doubling the size doubles the box
        // (within shaping rounding).
        let width_ratio = f32::from(large_m.cell_width) / f32::from(small_m.cell_width);
        let height_ratio = f32::from(large_m.cell_height) / f32::from(small_m.cell_height);
        assert!(
            (width_ratio - 2.0).abs() < 0.05,
            "width ratio {width_ratio} is not ~2.0"
        );
        assert!(
            (height_ratio - 2.0).abs() < 0.05,
            "height ratio {height_ratio} is not ~2.0"
        );
    }
}
