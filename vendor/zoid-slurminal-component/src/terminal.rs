//! Terminal emulator core wrapping `libghostty-vt`.
//!
//! This module owns the safe Rust interface over the VT core. It re-exports
//! the key types from `libghostty_vt` and provides Zoid-specific extensions
//! (callbacks, theme integration) that the GPUI surface layer consumes.

pub use libghostty_vt::Error;
pub use libghostty_vt::RenderState;
pub use libghostty_vt::fmt::{Format, Formatter, FormatterOptions};
pub use libghostty_vt::render::{
    CellIterator, Colors, CursorViewport, CursorVisualStyle, Dirty, RowIterator,
};
pub use libghostty_vt::screen::Screen;
pub use libghostty_vt::screen::{Cell, CellWide, GridRef, Row, TrackedGridRef};
pub use libghostty_vt::selection::{SelectLineOptions, SelectWordOptions, Selection};
pub use libghostty_vt::style::Palette;
pub use libghostty_vt::style::PaletteIndex;
pub use libghostty_vt::style::RgbColor;
pub use libghostty_vt::style::{Style, StyleColor, Underline};
pub use libghostty_vt::terminal::ScrollViewport;
pub use libghostty_vt::terminal::Scrollbar;
pub use libghostty_vt::terminal::Terminal as VtTerminal;
pub use libghostty_vt::terminal::{
    ClipboardContent, ClipboardLocation, ClipboardWrite, ClipboardWriteError, CursorStyle, Mode,
    Point, PointCoordinate, PointSpace, SizeReportSize,
};

/// Configuration for creating a new terminal instance.
pub struct TerminalConfig {
    /// Number of columns.
    pub cols: u16,
    /// Number of rows.
    pub rows: u16,
    /// Maximum scrollback lines.
    pub max_scrollback: usize,
    /// Default foreground color.
    pub default_fg: RgbColor,
    /// Default background color.
    pub default_bg: RgbColor,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            max_scrollback: 10_000,
            default_fg: RgbColor {
                r: 0xDD,
                g: 0xDD,
                b: 0xDD,
            },
            default_bg: RgbColor {
                r: 0x1E,
                g: 0x1E,
                b: 0x2E,
            },
        }
    }
}

/// A terminal emulator instance.
///
/// Wraps [`VtTerminal`] with Zoid-specific configuration. All operations
/// are `!Send + !Sync` by design — drive from the main thread only.
pub struct Terminal {
    inner: VtTerminal<'static, 'static>,
    config: TerminalConfig,
}

impl Terminal {
    /// Create a new terminal with the given configuration.
    ///
    /// Scrollback is configured via the `set_scrollback_max_lines` setter:
    /// the new C API era (`Terminal::new(cols, rows)`) carries no options
    /// struct.
    pub fn new(config: TerminalConfig) -> Result<Self, Error> {
        let mut inner = VtTerminal::new(config.cols, config.rows)?;
        inner.set_scrollback_max_lines(Some(config.max_scrollback))?;
        inner.set_default_fg_color(Some(config.default_fg))?;
        inner.set_default_bg_color(Some(config.default_bg))?;
        Ok(Self { inner, config })
    }

    /// Feed VT-encoded bytes into the terminal.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.inner.vt_write(bytes);
    }

    /// Resize the terminal to the given dimensions.
    ///
    /// `cell_width_px` and `cell_height_px` are the pixel dimensions of a
    /// single cell, used for image protocols and size reports. Pass 0 if
    /// unknown.
    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Error> {
        self.config.cols = cols;
        self.config.rows = rows;
        self.inner.resize(cols, rows, cell_width_px, cell_height_px)
    }

    /// Get the cursor column position (inner-indexed).
    pub fn cursor_x(&self) -> Result<u16, Error> {
        self.inner.cursor_x()
    }

    /// Get the cursor row position within the active area (inner-indexed).
    pub fn cursor_y(&self) -> Result<u16, Error> {
        self.inner.cursor_y()
    }

    /// Get the cursor position as (col, row).
    pub fn cursor_position(&self) -> Result<(u16, u16), Error> {
        Ok((self.inner.cursor_x()?, self.inner.cursor_y()?))
    }

    /// Get whether the cursor is visible.
    pub fn is_cursor_visible(&self) -> Result<bool, Error> {
        self.inner.is_cursor_visible()
    }

    /// Get the terminal title, if set by the running program.
    pub fn title(&self) -> Result<&str, Error> {
        self.inner.title()
    }

    /// Get the current working directory, if set by escape sequences.
    pub fn pwd(&self) -> Result<&str, Error> {
        self.inner.pwd()
    }

    /// Scroll the terminal viewport.
    pub fn scroll_viewport(&mut self, scroll: ScrollViewport) {
        self.inner.scroll_viewport(scroll);
    }

    /// Get the scrollbar state for the terminal viewport.
    pub fn scrollbar(&self) -> Result<Scrollbar, Error> {
        self.inner.scrollbar()
    }

    /// Get the terminal width in cells.
    pub fn cols(&self) -> Result<u16, Error> {
        self.inner.cols()
    }

    /// Get the terminal height in cells.
    pub fn rows(&self) -> Result<u16, Error> {
        self.inner.rows()
    }

    /// Get the number of scrollback rows.
    pub fn scrollback_rows(&self) -> Result<usize, Error> {
        self.inner.scrollback_rows()
    }

    /// Get whether any mouse tracking mode is active.
    pub fn is_mouse_tracking(&self) -> Result<bool, Error> {
        self.inner.is_mouse_tracking()
    }

    /// Get the currently active screen.
    pub fn active_screen(&self) -> Result<Screen, Error> {
        self.inner.active_screen()
    }

    /// Get the effective foreground color.
    pub fn fg_color(&self) -> Result<Option<RgbColor>, Error> {
        self.inner.fg_color()
    }

    /// Get the effective background color.
    pub fn bg_color(&self) -> Result<Option<RgbColor>, Error> {
        self.inner.bg_color()
    }

    /// Get the effective cursor color.
    pub fn cursor_color(&self) -> Result<Option<RgbColor>, Error> {
        self.inner.cursor_color()
    }

    /// Get the current 256-color palette.
    pub fn color_palette(&self) -> Result<Palette, Error> {
        self.inner.color_palette()
    }

    /// Get the value of a terminal mode.
    pub fn mode(&self, mode: Mode) -> Result<bool, Error> {
        self.inner.mode(mode)
    }

    /// Set the value of a terminal mode.
    pub fn set_mode(&mut self, mode: Mode, value: bool) -> Result<(), Error> {
        self.inner.set_mode(mode, value)?;
        Ok(())
    }

    /// Perform a full terminal reset (RIS).
    pub fn reset(&mut self) -> Result<(), Error> {
        self.inner.reset();
        Ok(())
    }

    /// Override the default foreground color on a live terminal.
    ///
    /// Used by the surface theme layer (9.2.5) to re-theme without
    /// recreating the terminal. Pass `None` to restore Ghostty's default.
    pub fn set_default_fg_color(&mut self, color: Option<RgbColor>) -> Result<(), Error> {
        self.inner.set_default_fg_color(color)?;
        Ok(())
    }

    /// Override the default background color on a live terminal.
    ///
    /// See [`Terminal::set_default_fg_color`].
    pub fn set_default_bg_color(&mut self, color: Option<RgbColor>) -> Result<(), Error> {
        self.inner.set_default_bg_color(color)?;
        Ok(())
    }

    /// Override the default cursor color on a live terminal.
    ///
    /// See [`Terminal::set_default_fg_color`].
    pub fn set_default_cursor_color(&mut self, color: Option<RgbColor>) -> Result<(), Error> {
        self.inner.set_default_cursor_color(color)?;
        Ok(())
    }

    /// Override the full 256-color palette on a live terminal.
    ///
    /// See [`Terminal::set_default_fg_color`]. Build the replacement from
    /// [`Palette::default`] plus per-index [`Palette::set`] so entries the
    /// theme does not name keep Ghostty's built-ins.
    pub fn set_default_color_palette(&mut self, palette: Option<Palette>) -> Result<(), Error> {
        self.inner.set_default_color_palette(palette)?;
        Ok(())
    }

    /// Cap the scrollback buffer on a live terminal.
    ///
    /// Pass `None` for an unbounded buffer. Applied at view construction
    /// from the ZML `scrollback_limit` prop (9.3.1).
    pub fn set_scrollback_max_lines(&mut self, max: Option<usize>) -> Result<(), Error> {
        self.inner.set_scrollback_max_lines(max)?;
        Ok(())
    }

    // --- Effect registration ---
    // The libghostty-vt callbacks take &Terminal (immutable), which is
    // unusual. We delegate directly.

    /// Register a callback for PTY write-back (query responses).
    pub fn on_pty_write(
        &mut self,
        f: impl FnMut(&VtTerminal<'static, 'static>, &[u8]) + 'static,
    ) -> Result<(), Error> {
        self.inner.on_pty_write(f)?;
        Ok(())
    }

    /// Register a callback for bell events.
    pub fn on_bell(
        &mut self,
        f: impl FnMut(&VtTerminal<'static, 'static>) + 'static,
    ) -> Result<(), Error> {
        self.inner.on_bell(f)?;
        Ok(())
    }

    /// Register a callback for title changes.
    pub fn on_title_changed(
        &mut self,
        f: impl FnMut(&VtTerminal<'static, 'static>) + 'static,
    ) -> Result<(), Error> {
        self.inner.on_title_changed(f)?;
        Ok(())
    }

    /// Borrow the inner VT terminal for rendering.
    pub fn inner(&self) -> &VtTerminal<'static, 'static> {
        &self.inner
    }

    /// Mutably borrow the inner VT terminal for effect registration.
    ///
    /// Use this to register callbacks like `on_clipboard_write` that are
    /// not yet wrapped by this crate's API.
    pub fn inner_mut(&mut self) -> &mut VtTerminal<'static, 'static> {
        &mut self.inner
    }

    /// Read a single viewport cell as owned text plus style.
    ///
    /// `col` is 0-indexed; `row` is the 0-indexed viewport row (it moves
    /// when scrolled, unlike [`Point::Screen`] rows).
    pub fn cell_at(&self, col: u16, row: u32) -> Result<GridCell, Error> {
        let grid = self
            .inner
            .grid_ref(Point::Viewport(PointCoordinate { x: col, y: row }))?;
        Ok(GridCell {
            text: grid_text(&grid)?,
            style: grid.style()?,
            hyperlink: grid_has_hyperlink(&grid),
        })
    }

    /// Read one viewport row as plain text (no trailing newline).
    pub fn row_text(&self, row: u32) -> Result<String, Error> {
        let cols = self.cols()?;
        let mut text = String::new();
        for col in 0..cols {
            text.push_str(&self.cell_at(col, row)?.text);
        }
        Ok(text.trim_end().to_string())
    }

    /// Read the whole visible viewport as plain text, rows joined by `\n`.
    ///
    /// This is the cheap path for tests and copy-all. The 60fps render
    /// path (task 9.2.1) uses [`Terminal::snapshot`] instead.
    pub fn visible_text(&self) -> Result<String, Error> {
        format_plain(&self.inner, None)
    }

    /// Pin a viewport cell to a tracked reference that survives terminal
    /// mutations.
    ///
    /// Plain [`GridRef`]s go stale on the next mutating call. Use the
    /// tracked ref to keep selection endpoints valid across output, then
    /// re-resolve with [`TrackedGridRef::snapshot`] before reading.
    pub fn track_point(&self, col: u16, row: u32) -> Result<TrackedGridRef, Error> {
        self.inner
            .track_grid_ref(Point::Viewport(PointCoordinate { x: col, y: row }))
    }

    /// Capture an owned snapshot of the visible viewport: rows of styled
    /// cells plus cursor, colors, and dirty state.
    ///
    /// Built on the render-state API (dirty-tracked, palette-resolved),
    /// so this is what the GPUI surface reads once per frame.
    pub fn snapshot(&self) -> Result<OwnedSnapshot, Error> {
        let mut state = RenderState::new()?;
        let snap = state.update(&self.inner)?;
        let mut rows_data = Vec::new();
        let mut row_iter = RowIterator::new()?;
        let mut cell_iter = CellIterator::new()?;
        let mut row_it = row_iter.update(&snap)?;
        while let Some(row) = row_it.next() {
            let wrapped = row.raw_row()?.is_wrapped()?;
            let mut cell_it = cell_iter.update(row)?;
            let mut cells = Vec::new();
            while let Some(cell) = cell_it.next() {
                let text: String = cell.graphemes()?.into_iter().collect();
                cells.push(SnapshotCell {
                    text,
                    style: cell.style()?,
                    fg: cell.fg_color()?,
                    bg: cell.bg_color()?,
                    selected: cell.is_selected()?,
                });
            }
            rows_data.push(SnapshotRow { cells, wrapped });
        }
        Ok(OwnedSnapshot {
            cols: snap.cols()?,
            rows: snap.rows()?,
            rows_data,
            dirty: snap.dirty()?,
            colors: snap.colors()?,
            cursor_visible: snap.cursor_visible()?,
            cursor_blinking: snap.cursor_blinking()?,
            cursor_style: snap.cursor_visual_style()?,
            cursor: snap.cursor_viewport()?,
        })
    }

    /// Select everything and return it as plain text.
    ///
    /// Returns `Ok(None)` when the terminal holds no selectable content.
    /// Does not disturb the terminal-owned selection: the selection is
    /// formatted directly instead of being installed first.
    pub fn select_all_text(&self) -> Result<Option<String>, Error> {
        let Some(sel) = self.inner.select_all()? else {
            return Ok(None);
        };
        Ok(Some(format_plain(&self.inner, Some(&sel))?))
    }

    /// Select the logical line under a viewport cell, return plain text.
    ///
    /// Returns `Ok(None)` when nothing is selectable there.
    pub fn select_line_text(&self, col: u16, row: u32) -> Result<Option<String>, Error> {
        let grid = self
            .inner
            .grid_ref(Point::Viewport(PointCoordinate { x: col, y: row }))?;
        let opts = SelectLineOptions::new(grid);
        let Some(sel) = self.inner.select_line(opts)? else {
            return Ok(None);
        };
        Ok(Some(format_plain(&self.inner, Some(&sel))?))
    }

    /// Clear the terminal-owned selection, if any.
    pub fn clear_selection(&self) -> Result<(), Error> {
        self.inner.set_selection(None)?;
        Ok(())
    }

    /// Select the word under a viewport cell, return plain text.
    ///
    /// Returns `Ok(None)` when nothing is selectable there. Mirrors
    /// [`Terminal::select_line_text`]; the word breaks on whitespace and
    /// punctuation per the terminal's word definition.
    pub fn select_word_text(&self, col: u16, row: u32) -> Result<Option<String>, Error> {
        let grid = self
            .inner
            .grid_ref(Point::Viewport(PointCoordinate { x: col, y: row }))?;
        let opts = SelectWordOptions::new(grid);
        let Some(sel) = self.inner.select_word(opts)? else {
            return Ok(None);
        };
        Ok(Some(format_plain(&self.inner, Some(&sel))?))
    }

    /// Format an arbitrary selection snapshot as plain text.
    ///
    /// Used by the surface selection state (9.2.4): the endpoints are
    /// resolved from tracked refs, so the selection is never installed on
    /// the terminal and output arriving underneath it does not disturb it.
    pub fn selection_text(&self, selection: &Selection<'_>) -> Result<String, Error> {
        format_plain(&self.inner, Some(selection))
    }

    /// Install a selection snapshot as the terminal-owned selection.
    ///
    /// Installed selections surface as `selected` flags in
    /// [`OwnedSnapshot`] cells, which the grid renderer draws inverse until
    /// 9.2.5 supplies a theme selection color. Pass `None` to uninstall.
    pub fn set_selection(&self, selection: Option<&Selection<'_>>) -> Result<(), Error> {
        self.inner.set_selection(selection)?;
        Ok(())
    }

    /// Read a single screen-space cell as owned text.
    ///
    /// Unlike [`Terminal::cell_at`] (viewport rows, which move when
    /// scrolled), `screen_row` addresses the full screen including
    /// scrollback: row 0 is the oldest line, `total_rows - 1` the newest.
    /// Used by block-copy and search, which must reach history.
    pub fn screen_cell_text(&self, col: u16, screen_row: u32) -> Result<String, Error> {
        let grid = self.inner.grid_ref(Point::Screen(PointCoordinate {
            x: col,
            y: screen_row,
        }))?;
        grid_text(&grid)
    }

    /// Read one screen-space row as plain text (no trailing newline).
    ///
    /// Screen-space variant of [`Terminal::row_text`] for history reads.
    pub fn screen_row_text(&self, screen_row: u32) -> Result<String, Error> {
        let cols = self.cols()?;
        let mut text = String::new();
        for col in 0..cols {
            text.push_str(&self.screen_cell_text(col, screen_row)?);
        }
        Ok(text.trim_end().to_string())
    }

    /// The SGR style active at the cursor (bold, fg/bg, underline, ...).
    ///
    /// This is the *text* style, not the cursor shape; the shape lives on
    /// [`OwnedSnapshot::cursor_style`] via the render state.
    pub fn active_style(&self) -> Result<Style, Error> {
        self.inner.cursor_style()
    }

    /// Register a callback for OSC 52 clipboard writes.
    ///
    /// The app performs the platform clipboard operation; the component
    /// only detects the sequence. Return `Ok(())` once stored.
    pub fn on_clipboard_write(
        &mut self,
        f: impl FnMut(
            &VtTerminal<'static, 'static>,
            ClipboardWrite<'_>,
        ) -> Result<(), ClipboardWriteError>
        + 'static,
    ) -> Result<(), Error> {
        self.inner.on_clipboard_write(f)?;
        Ok(())
    }

    /// Register a callback for XTWINOPS size queries (CSI 14/16/18 t).
    ///
    /// Return the reported size, or `None` to leave the query unanswered
    /// (the terminal falls back to its default reply).
    pub fn on_size_report(
        &mut self,
        f: impl FnMut(&VtTerminal<'static, 'static>) -> Option<SizeReportSize> + 'static,
    ) -> Result<(), Error> {
        self.inner.on_size(f)?;
        Ok(())
    }
}

/// One viewport cell: owned text plus its resolved style.
///
/// `text` is the full grapheme cluster (empty for blank cells). Wide-cell
/// spacer tails surface as empty text; the surface skips them when drawing.
pub struct GridCell {
    /// Grapheme cluster text of the cell.
    pub text: String,
    /// SGR style active for the cell.
    pub style: Style,
    /// Whether the cell carries an OSC 8 hyperlink.
    pub hyperlink: bool,
}

/// One render-state cell: owned text, style, resolved colors, selection.
pub struct SnapshotCell {
    /// Grapheme cluster text of the cell.
    pub text: String,
    /// SGR style active for the cell.
    pub style: Style,
    /// Resolved foreground color (`None` means the terminal default).
    pub fg: Option<RgbColor>,
    /// Resolved background color (`None` means the terminal default).
    pub bg: Option<RgbColor>,
    /// Whether the cell falls inside the terminal-owned selection.
    pub selected: bool,
}

/// One viewport row of a snapshot.
pub struct SnapshotRow {
    /// Cells left to right across the viewport width.
    pub cells: Vec<SnapshotCell>,
    /// Whether this row is soft-wrapped onto the next one.
    pub wrapped: bool,
}

/// Owned snapshot of the visible viewport for one frame.
///
/// Produced by [`Terminal::snapshot`] from the render-state API: dirty
/// tracking is palette-resolved and per-row, so the surface can skip
/// clean frames (`dirty == Dirty::Clean`) and redraw only dirty rows.
pub struct OwnedSnapshot {
    /// Viewport width in cells.
    pub cols: u16,
    /// Viewport height in cells.
    pub rows: u16,
    /// Row-major viewport contents.
    pub rows_data: Vec<SnapshotRow>,
    /// Frame dirty state at capture time.
    pub dirty: Dirty,
    /// Resolved default colors plus the 256-color palette.
    pub colors: Colors,
    /// Whether the cursor is visible per terminal modes.
    pub cursor_visible: bool,
    /// Whether the cursor blinks per terminal modes.
    pub cursor_blinking: bool,
    /// Visual shape of the cursor (bar/block/underline/hollow).
    pub cursor_style: CursorVisualStyle,
    /// Cursor position in viewport cells (`None` when off-viewport).
    pub cursor: Option<CursorViewport>,
}

impl OwnedSnapshot {
    /// Plain text of one viewport row, trailing blanks trimmed.
    #[must_use]
    pub fn row_text(&self, y: usize) -> String {
        self.rows_data
            .get(y)
            .map(|row| {
                let text: String = row.cells.iter().map(|c| c.text.as_str()).collect();
                text.trim_end().to_string()
            })
            .unwrap_or_default()
    }

    /// Plain text of the whole viewport, rows joined by `\n`.
    #[must_use]
    pub fn visible_text(&self) -> String {
        (0..self.rows_data.len())
            .map(|y| self.row_text(y))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Read a grid ref's grapheme cluster into an owned string.
///
/// Retries once with a correctly sized buffer when the cluster does not
/// fit the stack buffer (combining sequences, emoji ZWJ chains).
fn grid_text(grid: &GridRef<'_>) -> Result<String, Error> {
    let mut buf = ['\0'; 16];
    match grid.graphemes(&mut buf) {
        Ok(n) => Ok(buf[..n].iter().collect()),
        Err(Error::OutOfSpace { required }) => {
            let mut big = vec!['\0'; required];
            let n = grid.graphemes(&mut big)?;
            Ok(big[..n].iter().collect())
        }
        Err(e) => Err(e),
    }
}

/// Whether a grid ref carries an OSC 8 hyperlink.
///
/// Probes with a small stack buffer: any URI bytes (or an `OutOfSpace`
/// overflow, meaning the URI exceeds the probe) counts as a link.
fn grid_has_hyperlink(grid: &GridRef<'_>) -> bool {
    let mut buf = [0u8; 8];
    match grid.hyperlink_uri(&mut buf) {
        Ok(0) => false,
        Ok(_) => true,
        Err(Error::OutOfSpace { .. }) => true,
        Err(_) => false,
    }
}

/// Format the active screen as plain text, optionally restricted to a
/// selection snapshot. Never installs the selection on the terminal.
fn format_plain(
    terminal: &VtTerminal<'static, 'static>,
    selection: Option<&Selection<'_>>,
) -> Result<String, Error> {
    let mut opts = FormatterOptions::new().with_format(Format::Plain);
    if let Some(sel) = selection {
        opts = opts.with_selection(sel);
    }
    let mut fmt = Formatter::new(terminal, opts)?;
    let len = fmt.format_len()?;
    let mut buf = vec![0u8; len];
    let n = fmt.format_buf(&mut buf)?;
    buf.truncate(n);
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_terminal(cols: u16, rows: u16) -> Result<Terminal, Error> {
        Terminal::new(TerminalConfig {
            cols,
            rows,
            max_scrollback: 100,
            ..TerminalConfig::default()
        })
    }

    #[test]
    fn plain_text_feeds_grid_and_moves_cursor() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"Hello");
        assert_eq!(term.cell_at(0, 0)?.text, "H");
        assert_eq!(term.row_text(0)?, "Hello");
        assert_eq!(term.cursor_position()?, (5, 0));
        Ok(())
    }

    #[test]
    fn sgr_bold_marks_cell_style() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"\x1b[1;32mHi\x1b[0m");
        let cell = term.cell_at(0, 0)?;
        assert_eq!(cell.text, "H");
        assert!(cell.style.bold);
        let plain = term.cell_at(2, 0)?;
        assert!(!plain.style.bold);
        Ok(())
    }

    #[test]
    fn cursor_hide_sequence_clears_visibility_and_mode() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        assert!(term.is_cursor_visible()?);
        term.feed(b"\x1b[?25l");
        assert!(!term.is_cursor_visible()?);
        assert!(!term.mode(Mode::CURSOR_VISIBLE)?);
        term.feed(b"\x1b[?25h");
        assert!(term.is_cursor_visible()?);
        Ok(())
    }

    #[test]
    fn resize_updates_dims_and_grows_scrollback() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        for i in 0..10 {
            let line = format!("line{i}\r\n");
            term.feed(line.as_bytes());
        }
        assert!(term.scrollback_rows()? > 0);
        term.resize(40, 6, 0, 0)?;
        assert_eq!(term.cols()?, 40);
        assert_eq!(term.rows()?, 6);
        Ok(())
    }

    #[test]
    fn select_all_returns_fed_lines() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"alpha\r\nbeta");
        let text = term.select_all_text()?.unwrap_or_default();
        assert!(text.contains("alpha"), "select-all missed line 1: {text:?}");
        assert!(text.contains("beta"), "select-all missed line 2: {text:?}");
        term.clear_selection()?;
        Ok(())
    }

    #[test]
    fn snapshot_matches_grid_reads() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"Hello\r\nWorld");
        let snap = term.snapshot()?;
        assert_eq!(snap.cols, 20);
        assert_eq!(snap.rows, 4);
        assert_eq!(snap.rows_data.len(), 4);
        assert_eq!(snap.row_text(0), term.row_text(0)?);
        assert_eq!(snap.row_text(1), "World");
        assert_eq!(snap.visible_text(), "Hello\nWorld\n\n");
        assert!(snap.cursor_visible);
        let cursor = snap.cursor.unwrap_or(CursorViewport {
            x: 0,
            y: 0,
            at_wide_tail: false,
        });
        assert_eq!((cursor.x, cursor.y), (5, 1));
        Ok(())
    }

    #[test]
    fn osc8_marks_cell_hyperlink() -> Result<(), Error> {
        let mut term = test_terminal(30, 4)?;
        term.feed(b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\");
        assert!(term.cell_at(0, 0)?.hyperlink);
        assert!(!term.cell_at(5, 0)?.hyperlink);
        Ok(())
    }
}
