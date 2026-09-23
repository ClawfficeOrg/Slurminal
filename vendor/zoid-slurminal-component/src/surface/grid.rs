//! Grid render surface: [`OwnedSnapshot`] → resolved frame → GPUI element.
//!
//! Two layers, deliberately split:
//!
//! - [`GridFrame`] is pure data. It resolves every cell's colors (defaults,
//!   inverse, selection) and text attributes out of an [`OwnedSnapshot`]
//!   with no GPUI context, so fixtures assert on it with plain `#[test]`.
//! - [`TerminalGrid`] is the thin GPUI element over a [`GridFrame`]. One
//!   frame builds one element tree, so rapid output re-renders whole frames
//!   instead of patching cells — correct and flicker-free at 60fps by
//!   construction. When a damaged-region redraw lands, it consumes
//!   [`GridFrame::dirty`] plus the per-row [`ResolvedRow::dirty_hint`] and
//!   skips clean rows; no reshape of this module needed.

use gpui::{
    App, Font, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div,
};

use crate::terminal::{CursorVisualStyle, Dirty, OwnedSnapshot, RgbColor, SnapshotCell, Underline};

use super::{CellMetrics, SurfaceConfig, theme::TerminalPalette};

/// Convert a terminal RGB triple to a GPUI color.
///
/// `gpui::rgb` packs the same `0xRRGGBB` word, so the round trip is exact.
#[must_use]
pub fn rgb_to_gpui(color: RgbColor) -> Hsla {
    gpui::rgb(((color.r as u32) << 16) | ((color.g as u32) << 8) | (color.b as u32)).into()
}

/// One resolved cell: owned text plus final display attributes.
///
/// `faint`, `blink`, and `overline` are carried but not rendered yet —
/// GPUI offers no per-span faint/overline primitive and blink needs the
/// timer that 9.2.x input work owns. They ride along so rendering them
/// later touches one match arm, not the frame shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCell {
    /// Grapheme cluster text (`" "` when invisible, `""` for blank/tail).
    pub text: SharedString,
    /// Final foreground after defaults + inverse + selection.
    pub fg: Hsla,
    /// Final background after defaults + inverse + selection.
    pub bg: Hsla,
    /// SGR bold → heavy font weight.
    pub bold: bool,
    /// SGR italic → italic style.
    pub italic: bool,
    /// Any non-`None` underline SGR → underline decoration.
    pub underline: bool,
    /// SGR strikethrough → line-through decoration.
    pub strikethrough: bool,
    /// Carried for later; not rendered in 9.2.1.
    pub faint: bool,
    /// Carried for later; not rendered in 9.2.1.
    pub blink: bool,
    /// Carried for later; not rendered in 9.2.1.
    pub overline: bool,
    /// Inside the terminal-owned selection (rendered as inverse for now;
    /// 9.2.5 theme integration supplies a real selection color).
    pub selected: bool,
    /// The cursor sits on this cell.
    pub is_cursor: bool,
}

/// Resolve one snapshot cell against the frame defaults.
///
/// `selection_bg` carries the themed selection background (9.2.5) as a
/// terminal-native color: a selected cell takes it with no video swap.
/// `None` keeps the legacy 9.2.1 behaviour — selected renders as inverse —
/// so [`GridFrame::from_snapshot`] output is byte-identical to before.
fn resolve_cell(
    cell: &SnapshotCell,
    default_fg: RgbColor,
    default_bg: RgbColor,
    is_cursor: bool,
    selection_bg: Option<RgbColor>,
) -> ResolvedCell {
    let mut fg = cell.fg.unwrap_or(default_fg);
    let mut bg = cell.bg.unwrap_or(default_bg);
    // Inverse swaps video unconditionally now; selection applies after, so
    // the legacy `inverse != selected` XOR shape is preserved exactly when
    // `selection_bg` is `None` (swap-then-swap cancels).
    if cell.style.inverse {
        core::mem::swap(&mut fg, &mut bg);
    }
    if cell.selected {
        match selection_bg {
            Some(sel) => bg = sel,
            None => core::mem::swap(&mut fg, &mut bg),
        }
    }
    // A visible block cursor inverts its cell; other shapes are recorded on
    // the frame for 9.2.x and render as block until then.
    if is_cursor {
        core::mem::swap(&mut fg, &mut bg);
    }
    let text: SharedString = if cell.style.invisible {
        " ".into()
    } else {
        cell.text.as_str().into()
    };
    ResolvedCell {
        text,
        fg: rgb_to_gpui(fg),
        bg: rgb_to_gpui(bg),
        bold: cell.style.bold,
        italic: cell.style.italic,
        underline: cell.style.underline != Underline::None,
        strikethrough: cell.style.strikethrough,
        faint: cell.style.faint,
        blink: cell.style.blink,
        overline: cell.style.overline,
        selected: cell.selected,
        is_cursor,
    }
}

/// One resolved row plus its damaged-region hook.
///
/// `dirty_hint` is `None` until the snapshot path reports per-row dirty
/// bits (`RowIteration::dirty` exists on the render API; `OwnedSnapshot`
/// does not capture it yet). `None` means "assume dirty" — the future
/// incremental renderer treats it exactly like a dirty row.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRow {
    /// Cells left to right across the viewport width.
    pub cells: Vec<ResolvedCell>,
    /// Soft-wrapped onto the next row (from the snapshot).
    pub wrapped: bool,
    /// Per-row dirty hint for the future incremental renderer.
    pub dirty_hint: Option<bool>,
}

/// One atomic frame: everything [`TerminalGrid`] needs, nothing it must
/// borrow. Built from a single [`OwnedSnapshot`], so render and state can
/// never disagree mid-frame.
#[derive(Clone, Debug, PartialEq)]
pub struct GridFrame {
    /// Resolved rows in viewport order.
    pub rows: Vec<ResolvedRow>,
    /// Viewport width in cells.
    pub cols: u16,
    /// Viewport height in cells.
    pub rows_len: u16,
    /// Frame dirty state at capture; `Clean` means "skip the redraw".
    pub dirty: Dirty,
    /// Default background — the element's own background fill.
    pub background: Hsla,
    /// Cursor color when the terminal sets one, else default foreground.
    pub cursor_color: Hsla,
    /// Whether the cursor is visible per terminal modes.
    pub cursor_visible: bool,
    /// Cursor shape (recorded; v1 renders every shape as block inverse).
    pub cursor_style: CursorVisualStyle,
    /// Cursor viewport position (`None` when off-viewport).
    pub cursor: Option<(u16, u16)>,
}

impl GridFrame {
    /// Resolve a frame from a snapshot. Pure: no `App` or `Window` needed.
    ///
    /// Terminal-driven: defaults come from the snapshot (whatever the
    /// terminal holds) and selected cells render as inverse. Prefer
    /// [`GridFrame::from_snapshot_with`] when a [`TerminalPalette`] is in
    /// play — it renders the theme selection color instead.
    #[must_use]
    pub fn from_snapshot(snap: &OwnedSnapshot) -> Self {
        Self::resolve(snap, snap.colors.foreground, snap.colors.background, None)
    }

    /// Resolve a frame from a snapshot against a theme palette.
    ///
    /// Defaults come from the palette (equal to the snapshot's once the
    /// palette is applied to the terminal) and selected cells take the
    /// palette selection background. Pure like [`GridFrame::from_snapshot`].
    #[must_use]
    pub fn from_snapshot_with(snap: &OwnedSnapshot, palette: &TerminalPalette) -> Self {
        Self::resolve(
            snap,
            palette.foreground,
            palette.background,
            Some(palette.selection),
        )
    }

    /// Shared resolver: explicit defaults plus an optional themed
    /// selection background.
    fn resolve(
        snap: &OwnedSnapshot,
        default_fg: RgbColor,
        default_bg: RgbColor,
        selection_bg: Option<RgbColor>,
    ) -> Self {
        let cursor_pos = snap.cursor.map(|c| (c.x, c.y));
        let rows = snap
            .rows_data
            .iter()
            .enumerate()
            .map(|(y, row)| ResolvedRow {
                cells: row
                    .cells
                    .iter()
                    .enumerate()
                    .map(|(x, cell)| {
                        let is_cursor =
                            snap.cursor_visible && cursor_pos == Some((x as u16, y as u16));
                        resolve_cell(cell, default_fg, default_bg, is_cursor, selection_bg)
                    })
                    .collect(),
                wrapped: row.wrapped,
                dirty_hint: None,
            })
            .collect();
        Self {
            rows,
            cols: snap.cols,
            rows_len: snap.rows,
            dirty: snap.dirty,
            background: rgb_to_gpui(default_bg),
            cursor_color: rgb_to_gpui(snap.colors.cursor.unwrap_or(default_fg)),
            cursor_visible: snap.cursor_visible,
            cursor_style: snap.cursor_style,
            cursor: cursor_pos,
        }
    }

    /// Whether the snapshot reported no changes — the view may skip it.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        matches!(self.dirty, Dirty::Clean)
    }

    /// Plain text of one resolved row, trailing blanks trimmed.
    #[must_use]
    pub fn row_text(&self, y: usize) -> String {
        self.rows
            .get(y)
            .map(|row| {
                let text: String = row.cells.iter().map(|c| c.text.as_ref()).collect();
                text.trim_end().to_string()
            })
            .unwrap_or_default()
    }
}

/// GPUI element rendering one [`GridFrame`] as rows of fixed-size cells.
///
/// Each cell is a fixed `w × h` div (from [`CellMetrics`]) carrying its own
/// fg/bg plus bold/italic/underline/strikethrough. Fixed boxes keep columns
/// aligned even when glyphs differ in width; the font is monospace by
/// contract, so the advance matches the box.
#[derive(IntoElement)]
pub struct TerminalGrid {
    frame: GridFrame,
    metrics: CellMetrics,
    font: Font,
    font_size: gpui::Pixels,
}

impl TerminalGrid {
    /// Build an element from already-resolved parts.
    pub fn new(frame: GridFrame, metrics: CellMetrics, config: &SurfaceConfig) -> Self {
        Self {
            frame,
            metrics,
            font: super::terminal_font(config).font,
            font_size: config.font_size,
        }
    }

    /// Resolve `snap` and build the element in one step.
    pub fn from_snapshot(
        snap: &OwnedSnapshot,
        metrics: CellMetrics,
        config: &SurfaceConfig,
    ) -> Self {
        Self::new(GridFrame::from_snapshot(snap), metrics, config)
    }

    /// Resolve `snap` against a theme palette and build the element.
    ///
    /// Themed counterpart of [`TerminalGrid::from_snapshot`]: selected
    /// cells render the palette selection background.
    pub fn from_themed_snapshot(
        snap: &OwnedSnapshot,
        metrics: CellMetrics,
        config: &SurfaceConfig,
        palette: &TerminalPalette,
    ) -> Self {
        Self::new(
            GridFrame::from_snapshot_with(snap, palette),
            metrics,
            config,
        )
    }

    /// Row count of the underlying frame (for tests and layout).
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.frame.rows.len()
    }

    /// Whether the underlying frame reported no changes.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.frame.is_clean()
    }

    /// Borrow the resolved frame (for tests and future input hit-testing).
    #[must_use]
    pub fn frame(&self) -> &GridFrame {
        &self.frame
    }
}

impl RenderOnce for TerminalGrid {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let cell_w = self.metrics.cell_width;
        let cell_h = self.metrics.cell_height;
        div()
            .flex()
            .flex_col()
            .bg(self.frame.background)
            .font(self.font)
            .text_size(self.font_size)
            // Text boxes must match the row pitch: GPUI's default line
            // height (~1.6em) would push each baseline down and let the
            // next row's cell backgrounds paint over it.
            .line_height(cell_h)
            .children(self.frame.rows.into_iter().map(|row| {
                div().flex().flex_row().h(cell_h).children(
                    row.cells
                        .into_iter()
                        .map(|cell| {
                            let mut el = div()
                                .w(cell_w)
                                .h(cell_h)
                                .bg(cell.bg)
                                .text_color(cell.fg)
                                .whitespace_nowrap()
                                .child(cell.text);
                            if cell.bold {
                                el = el.font_weight(FontWeight::BOLD);
                            }
                            if cell.italic {
                                el = el.italic();
                            }
                            if cell.underline {
                                el = el.underline();
                            }
                            if cell.strikethrough {
                                el = el.line_through();
                            }
                            el
                        })
                        .collect::<Vec<_>>(),
                )
            }))
    }
}

#[cfg(test)]
mod tests {
    use crate::terminal::{Error, Terminal, TerminalConfig};

    use super::*;

    fn test_terminal(cols: u16, rows: u16) -> Result<Terminal, Error> {
        Terminal::new(TerminalConfig {
            cols,
            rows,
            max_scrollback: 100,
            ..TerminalConfig::default()
        })
    }

    fn frame_for_bytes(cols: u16, rows: u16, bytes: &[u8]) -> Result<GridFrame, Error> {
        let mut term = test_terminal(cols, rows)?;
        term.feed(bytes);
        Ok(GridFrame::from_snapshot(&term.snapshot()?))
    }

    #[test]
    fn fixture_renders_colors_and_text_attributes() -> Result<(), Error> {
        use crate::terminal::PaletteIndex;

        // Bold + palette-red foreground over two cells, then a plain cell.
        let mut term = test_terminal(20, 4)?;
        term.feed(b"\x1b[1;31mHi\x1b[0m!");
        let expected_red = term.color_palette()?.get(PaletteIndex::RED);
        let frame = GridFrame::from_snapshot(&term.snapshot()?);
        let styled = &frame.rows[0].cells[0];
        assert_eq!(styled.text.as_ref(), "H");
        assert!(styled.bold, "SGR 1 must set bold");
        assert_eq!(
            styled.fg,
            rgb_to_gpui(expected_red),
            "SGR 31 must resolve to the palette red entry"
        );
        let plain = &frame.rows[0].cells[2];
        assert_eq!(plain.text.as_ref(), "!");
        assert!(!plain.bold);
        Ok(())
    }

    #[test]
    fn sgr_underline_italic_strikethrough_resolve() -> Result<(), Error> {
        let frame = frame_for_bytes(20, 4, b"\x1b[3;4;9mX\x1b[0m")?;
        let cell = &frame.rows[0].cells[0];
        assert!(cell.italic, "SGR 3 must set italic");
        assert!(cell.underline, "SGR 4 must set underline");
        assert!(cell.strikethrough, "SGR 9 must set strikethrough");
        assert!(!frame.rows[0].cells[1].italic);
        Ok(())
    }

    #[test]
    fn inverse_swaps_fg_and_bg() -> Result<(), Error> {
        let frame = frame_for_bytes(20, 4, b"\x1b[7mI\x1b[0m")?;
        let cell = &frame.rows[0].cells[0];
        let snap_fg = RgbColor {
            r: 0xDD,
            g: 0xDD,
            b: 0xDD,
        };
        let snap_bg = RgbColor {
            r: 0x1E,
            g: 0x1E,
            b: 0x2E,
        };
        assert_eq!(cell.fg, rgb_to_gpui(snap_bg), "inverse swaps fg to bg");
        assert_eq!(cell.bg, rgb_to_gpui(snap_fg), "inverse swaps bg to fg");
        Ok(())
    }

    #[test]
    fn invisible_renders_blank_but_keeps_style() -> Result<(), Error> {
        let frame = frame_for_bytes(20, 4, b"\x1b[8msecret\x1b[0m")?;
        assert_eq!(frame.row_text(0), "", "invisible text must not show");
        Ok(())
    }

    #[test]
    fn fresh_output_frame_is_not_clean() -> Result<(), Error> {
        let frame = frame_for_bytes(20, 4, b"hello")?;
        assert!(!frame.is_clean(), "fresh output must mark the frame dirty");
        assert_eq!(frame.row_text(0), "hello");
        assert_eq!((frame.cols, frame.rows_len), (20, 4));
        Ok(())
    }

    #[test]
    fn cursor_cell_inverts_and_reports_position() -> Result<(), Error> {
        let frame = frame_for_bytes(20, 4, b"Hi")?;
        // Cursor sits after "Hi" at (2, 0); that cell inverts.
        assert_eq!(frame.cursor, Some((2, 0)));
        let under = &frame.rows[0].cells[2];
        assert!(under.is_cursor);
        assert!(!frame.rows[0].cells[0].is_cursor);
        Ok(())
    }

    #[gpui::test]
    fn element_builds_from_fixture_snapshot(cx: &mut gpui::TestAppContext) {
        use super::super::{SurfaceConfig, measure_cell_metrics};

        let frame = frame_for_bytes(20, 4, b"\x1b[1m$ \x1b[0mls");
        assert!(frame.is_ok(), "frame builds");
        let Ok(frame) = frame else { return };
        let (metrics, config) = cx.update(|cx| {
            let config = SurfaceConfig::default();
            (measure_cell_metrics(cx, &config), config)
        });
        let grid = TerminalGrid::new(frame, metrics, &config);
        assert_eq!(grid.row_count(), 4);
        assert!(!grid.is_clean());
        assert_eq!(grid.frame().row_text(0), "$ ls");
        assert!(grid.frame().rows[0].cells[0].bold);
    }
}
