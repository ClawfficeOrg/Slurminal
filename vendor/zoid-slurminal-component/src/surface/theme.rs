//! Terminal colors from the gpui-component theme (task 9.2.5).
//!
//! A terminal that ignores the app theme looks broken inside a themed Zoid
//! application, so the surface derives its colors from the active theme:
//! default foreground/background, the ANSI 16 palette, cursor color, and
//! selection color. An app can instead supply a standalone
//! [`TerminalPalette`] (what Slurminal needs for imported color schemes)
//! via [`PaletteSource::Custom`], which ignores the app theme entirely.
//!
//! The palette applies to a **live** terminal through the Ghostty default
//! setters — no recreation. The app flow is: build the palette (from the
//! theme or custom), [`TerminalPalette::apply_to`] once at creation, and
//! again after every [`Theme::change`]. Content, scrollback, and selection
//! survive; only colors move.
//!
//! ANSI 16 mapping from `ThemeColor` (gpui-component 0.6.6), documented
//! because the theme has no terminal-specific tokens:
//!
//! | Index | Name | Theme token |
//! |---|---|---|
//! | 0 | Black | `background` |
//! | 1–6 | Red/Green/Yellow/Blue/Magenta/Cyan | `red`/`green`/`yellow`/`blue`/`magenta`/`cyan` |
//! | 7 | White | `foreground` |
//! | 8 | BrightBlack | `muted_foreground` |
//! | 9–14 | Bright colors | `*_light` base hues |
//! | 15 | BrightWhite | `foreground` lightened by [`BRIGHT_WHITE_LIFT`] |
//!
//! Palette entries 16–255 keep Ghostty's built-ins. Cursor comes from
//! `caret`, selection background from `selection`.
//!
//! [`Theme::change`]: https://docs.rs/gpui-component/latest (see vendored
//! `gpui-component-0.6.6/src/theme/mod.rs::Theme::change`)

use gpui::{Hsla, Rgba};
use gpui_kit::component::ThemeColor;

use crate::terminal::{Error, Palette, PaletteIndex, RgbColor, Terminal};

/// Lightness lift applied to `foreground` for ANSI bright-white (index 15).
///
/// The theme exposes no white token distinct from `foreground`, so bright
/// white derives deterministically instead of colliding with index 7.
pub const BRIGHT_WHITE_LIFT: f32 = 0.12;

/// Terminal color set: everything the surface resolves from.
///
/// Terminal-native `RgbColor` throughout (what Ghostty consumes); the grid
/// renderer converts to `Hsla` at resolve time via `rgb_to_gpui`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalPalette {
    /// Default text color.
    pub foreground: RgbColor,
    /// Default cell background and element fill.
    pub background: RgbColor,
    /// Cursor color.
    pub cursor: RgbColor,
    /// Selected-cell background (replaces the inverse trick when themed).
    pub selection: RgbColor,
    /// ANSI 16 entries, index 0 (black) through 15 (bright white).
    pub ansi: [RgbColor; 16],
}

impl TerminalPalette {
    /// Derive the palette from gpui-component theme colors.
    ///
    /// Pure (no `App` needed): pass `&Theme::global(cx).colors` at the
    /// call site, or `&ThemeColor::dark()` / `&ThemeColor::light()` in
    /// tests. Re-derive after every theme change and [`apply_to`][Self::apply_to]
    /// again — the terminal is not recreated.
    #[must_use]
    pub fn from_theme_colors(colors: &ThemeColor) -> Self {
        let base = [
            colors.background,
            colors.red,
            colors.green,
            colors.yellow,
            colors.blue,
            colors.magenta,
            colors.cyan,
            colors.foreground,
            colors.muted_foreground,
            colors.red_light,
            colors.green_light,
            colors.yellow_light,
            colors.blue_light,
            colors.magenta_light,
            colors.cyan_light,
            lighten(colors.foreground, BRIGHT_WHITE_LIFT),
        ];
        Self {
            foreground: hsla_to_rgb(colors.foreground),
            background: hsla_to_rgb(colors.background),
            cursor: hsla_to_rgb(colors.caret),
            selection: hsla_to_rgb(colors.selection),
            ansi: base.map(hsla_to_rgb),
        }
    }

    /// Expand to Ghostty's 256-entry palette.
    ///
    /// Starts from [`Palette::default`] (Ghostty built-ins) and replaces
    /// entries 0–15, so colors the theme does not name survive untouched.
    #[must_use]
    pub fn to_ghostty_palette(&self) -> Palette {
        const INDICES: [PaletteIndex; 16] = [
            PaletteIndex::BLACK,
            PaletteIndex::RED,
            PaletteIndex::GREEN,
            PaletteIndex::YELLOW,
            PaletteIndex::BLUE,
            PaletteIndex::MAGENTA,
            PaletteIndex::CYAN,
            PaletteIndex::WHITE,
            PaletteIndex::BRIGHT_BLACK,
            PaletteIndex::BRIGHT_RED,
            PaletteIndex::BRIGHT_GREEN,
            PaletteIndex::BRIGHT_YELLOW,
            PaletteIndex::BRIGHT_BLUE,
            PaletteIndex::BRIGHT_MAGENTA,
            PaletteIndex::BRIGHT_CYAN,
            PaletteIndex::BRIGHT_WHITE,
        ];
        let mut palette = Palette::default();
        for (index, color) in INDICES.iter().zip(self.ansi.iter()) {
            palette.set(*index, *color);
        }
        palette
    }

    /// Apply the palette to a live terminal: defaults, cursor, and the full
    /// 256-entry palette. Content, scrollback, and modes are untouched.
    pub fn apply_to(&self, term: &mut Terminal) -> Result<(), Error> {
        term.set_default_fg_color(Some(self.foreground))?;
        term.set_default_bg_color(Some(self.background))?;
        term.set_default_cursor_color(Some(self.cursor))?;
        term.set_default_color_palette(Some(self.to_ghostty_palette()))?;
        Ok(())
    }
}

/// Where the surface takes its colors from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PaletteSource {
    /// Derive from the active gpui-component theme each time colors
    /// resolve. The app must re-apply after `Theme::change`.
    #[default]
    Theme,
    /// Standalone palette (e.g. an imported Slurminal color scheme).
    /// Ignores the app theme entirely.
    Custom(TerminalPalette),
}

/// Convert a theme color to terminal-native RGB, dropping alpha.
///
/// Uses GPUI's own `Hsla → Rgba` conversion so theme hues match the rest
/// of the app exactly; channels round to the nearest `u8`.
#[must_use]
pub fn hsla_to_rgb(color: Hsla) -> RgbColor {
    let rgba = Rgba::from(color);
    RgbColor {
        r: channel(rgba.r),
        g: channel(rgba.g),
        b: channel(rgba.b),
    }
}

/// Raise lightness by `amount`, clamped to valid range.
#[must_use]
pub fn lighten(color: Hsla, amount: f32) -> Hsla {
    Hsla {
        l: (color.l + amount).clamp(0.0, 1.0),
        ..color
    }
}

fn channel(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::ThemeColor;

    use super::*;
    use crate::terminal::TerminalConfig;

    fn test_terminal(cols: u16, rows: u16) -> Result<Terminal, Error> {
        Terminal::new(TerminalConfig {
            cols,
            rows,
            max_scrollback: 100,
            ..TerminalConfig::default()
        })
    }

    #[test]
    fn dark_theme_maps_documented_tokens() -> Result<(), Error> {
        let colors = ThemeColor::dark();
        let palette = TerminalPalette::from_theme_colors(&colors);
        assert_eq!(palette.foreground, hsla_to_rgb(colors.foreground));
        assert_eq!(palette.background, hsla_to_rgb(colors.background));
        assert_eq!(palette.cursor, hsla_to_rgb(colors.caret));
        assert_eq!(palette.selection, hsla_to_rgb(colors.selection));
        assert_eq!(palette.ansi[0], hsla_to_rgb(colors.background));
        assert_eq!(palette.ansi[1], hsla_to_rgb(colors.red));
        assert_eq!(palette.ansi[7], hsla_to_rgb(colors.foreground));
        assert_eq!(palette.ansi[8], hsla_to_rgb(colors.muted_foreground));
        assert_eq!(palette.ansi[9], hsla_to_rgb(colors.red_light));
        assert_eq!(
            palette.ansi[15],
            hsla_to_rgb(lighten(colors.foreground, BRIGHT_WHITE_LIFT))
        );
        assert_ne!(
            palette.ansi[15], palette.ansi[7],
            "bright white must not collide with white"
        );
        Ok(())
    }

    #[test]
    fn light_and_dark_themes_differ() {
        let dark = TerminalPalette::from_theme_colors(&ThemeColor::dark());
        let light = TerminalPalette::from_theme_colors(&ThemeColor::light());
        assert_ne!(dark.background, light.background);
        assert_ne!(dark.foreground, light.foreground);
    }

    #[test]
    fn ghostty_palette_replaces_ansi_sixteen_only() {
        let palette = TerminalPalette::from_theme_colors(&ThemeColor::dark());
        let ghostty = palette.to_ghostty_palette();
        let builtin = Palette::default();
        assert_eq!(ghostty.get(PaletteIndex::BLACK), palette.ansi[0]);
        assert_eq!(ghostty.get(PaletteIndex::RED), palette.ansi[1]);
        assert_eq!(ghostty.get(PaletteIndex::BRIGHT_WHITE), palette.ansi[15]);
        // Entries the theme does not name keep Ghostty's built-ins.
        for index in [16, 100, 231, 255] {
            let entry = PaletteIndex(index);
            assert_eq!(ghostty.get(entry), builtin.get(entry));
        }
    }

    #[test]
    fn apply_rethemes_live_terminal_without_recreation() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"kept");
        let (cols, rows) = (term.cols()?, term.rows()?);

        let dark = TerminalPalette::from_theme_colors(&ThemeColor::dark());
        dark.apply_to(&mut term)?;
        assert_eq!(term.fg_color()?.unwrap_or_default(), dark.foreground);
        assert_eq!(term.bg_color()?.unwrap_or_default(), dark.background);
        assert_eq!(term.cursor_color()?.unwrap_or_default(), dark.cursor);
        assert_eq!(term.color_palette()?.get(PaletteIndex::RED), dark.ansi[1]);

        // Re-theme in place: same instance, content and dims survive.
        let light = TerminalPalette::from_theme_colors(&ThemeColor::light());
        light.apply_to(&mut term)?;
        assert_eq!(term.fg_color()?.unwrap_or_default(), light.foreground);
        assert_eq!(term.bg_color()?.unwrap_or_default(), light.background);
        assert_eq!((term.cols()?, term.rows()?), (cols, rows));
        assert_eq!(term.row_text(0)?, "kept");
        Ok(())
    }

    #[test]
    fn custom_palette_overrides_theme() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        let custom = TerminalPalette {
            foreground: RgbColor { r: 1, g: 2, b: 3 },
            background: RgbColor { r: 4, g: 5, b: 6 },
            cursor: RgbColor { r: 7, g: 8, b: 9 },
            selection: RgbColor {
                r: 10,
                g: 11,
                b: 12,
            },
            ansi: [RgbColor {
                r: 13,
                g: 14,
                b: 15,
            }; 16],
        };
        custom.apply_to(&mut term)?;
        assert_eq!(term.fg_color()?.unwrap_or_default(), custom.foreground);
        assert_eq!(term.bg_color()?.unwrap_or_default(), custom.background);
        let dark = TerminalPalette::from_theme_colors(&ThemeColor::dark());
        assert_ne!(custom.foreground, dark.foreground);
        Ok(())
    }

    #[test]
    fn themed_selection_uses_selection_color() -> Result<(), Error> {
        use crate::surface::grid::{GridFrame, rgb_to_gpui};
        use crate::surface::selection::{CellPoint, SelectionState};

        let mut term = test_terminal(20, 4)?;
        term.feed(b"hi");
        let palette = TerminalPalette::from_theme_colors(&ThemeColor::dark());
        palette.apply_to(&mut term)?;
        let mut sel = SelectionState::new();
        sel.begin(&term, CellPoint { col: 0, row: 0 })?;
        sel.extend(&term, CellPoint { col: 1, row: 0 })?;
        sel.install(&term)?;

        let frame = GridFrame::from_snapshot_with(&term.snapshot()?, &palette);
        assert_eq!(
            frame.rows[0].cells[0].bg,
            rgb_to_gpui(palette.selection),
            "themed selected cell takes the selection background"
        );
        assert_eq!(
            frame.rows[0].cells[5].bg,
            rgb_to_gpui(palette.background),
            "unselected cell keeps the themed background"
        );
        // Legacy path still renders selected as inverse, not themed.
        let legacy = GridFrame::from_snapshot(&term.snapshot()?);
        assert_eq!(
            legacy.rows[0].cells[0].bg,
            rgb_to_gpui(palette.foreground),
            "legacy selected cell inverts to the foreground"
        );
        Ok(())
    }

    #[gpui::test]
    fn themed_frame_uses_palette_and_selection(cx: &mut gpui::TestAppContext) {
        use crate::surface::grid::{GridFrame, rgb_to_gpui};

        let _ = cx;
        let Ok(mut term) = test_terminal(20, 4) else {
            return;
        };
        term.feed(b"hi");
        let palette = TerminalPalette::from_theme_colors(&ThemeColor::dark());
        if palette.apply_to(&mut term).is_err() {
            return;
        }
        let Ok(snap) = term.snapshot() else { return };
        let frame = GridFrame::from_snapshot_with(&snap, &palette);
        assert_eq!(frame.background, rgb_to_gpui(palette.background));
        assert_eq!(
            frame.cursor_color,
            rgb_to_gpui(palette.cursor),
            "cursor color comes from the theme caret token"
        );
        // Plain cell resolves against the themed defaults.
        assert_eq!(frame.rows[0].cells[0].fg, rgb_to_gpui(palette.foreground));
    }

    #[gpui::test]
    fn global_theme_change_rethemes_without_recreation(cx: &mut gpui::TestAppContext) {
        use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode};

        cx.update(|cx| {
            gpui_kit::init(cx);
            let dark = TerminalPalette::from_theme_colors(&cx.theme().colors);
            let Ok(mut term) = test_terminal(20, 4) else {
                return;
            };
            term.feed(b"kept");
            if dark.apply_to(&mut term).is_err() {
                return;
            }
            let Ok(before) = term.snapshot() else { return };

            Theme::change(ThemeMode::Dark, None, cx);
            let light = TerminalPalette::from_theme_colors(&cx.theme().colors);
            if light.apply_to(&mut term).is_err() {
                return;
            }
            let Ok(after) = term.snapshot() else { return };

            assert_ne!(
                before.colors.background, after.colors.background,
                "theme change must move the live terminal background"
            );
            assert_eq!(after.row_text(0), "kept", "content survives re-theme");
        });
    }
}
