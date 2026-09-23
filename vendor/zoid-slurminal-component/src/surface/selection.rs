//! Mouse selection, scrollback navigation, and scroll position (task 9.2.4).
//!
//! Selection endpoints are [`TrackedGridRef`]s pinned with
//! [`Terminal::track_point`], so they survive output arriving underneath
//! the selection: this module's policy is **selection survives output**.
//! The selection is resolved to a [`Selection`] snapshot only when read
//! (copy/install), never installed eagerly, so a feed can neither disturb
//! nor clear it. Only an explicit [`SelectionState::clear`] (or a new
//! [`SelectionState::begin`]) ends it. Documented per the task contract,
//! which requires picking one behaviour.
//!
//! Scrollback navigation rides the terminal's own viewport
//! ([`ScrollViewport`]): wheel/keyboard deltas scroll history, and the view
//! returns to live output on input via [`snap_to_live`]. [`ScrollIndicator`]
//! derives the scrollback position indicator from [`Scrollbar`] —
//! `offset + len >= total` means pinned to live.
//!
//! Everything here is plain Rust with no GPUI dependency (same rule as
//! `surface::keys`/`surface::input`): GPUI event wiring waits for 9.3.

use crate::terminal::{
    Error, PointCoordinate, PointSpace, ScrollViewport, Selection, Terminal, TrackedGridRef,
};

/// One viewport cell address, as delivered by mouse events.
///
/// Viewport rows move when scrolled; the tracked refs pinned at `begin`
/// follow the content, so a stored `CellPoint` is only meaningful at the
/// moment it is passed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellPoint {
    /// 0-indexed column.
    pub col: u16,
    /// 0-indexed viewport row.
    pub row: u32,
}

/// App-side mouse selection with tracked endpoints.
///
/// Linear by default; [`SelectionState::set_rectangle`] switches to block
/// selection (opposite corners of a rectangle). Copy of a linear selection
/// goes through the terminal formatter (wrapping-aware); block copy reads
/// the cell rectangle directly.
pub struct SelectionState {
    anchor: Option<TrackedAnchor>,
}

struct TrackedAnchor {
    start: TrackedGridRef,
    end: TrackedGridRef,
    rectangle: bool,
}

impl SelectionState {
    /// Empty selection (nothing selected).
    #[must_use]
    pub fn new() -> Self {
        Self { anchor: None }
    }

    /// Whether a selection is currently held.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.anchor.is_some()
    }

    /// Start a drag at a viewport cell, pinning both endpoints there.
    ///
    /// Replaces any previous selection.
    pub fn begin(&mut self, term: &Terminal, at: CellPoint) -> Result<(), Error> {
        let start = term.track_point(at.col, at.row)?;
        let end = term.track_point(at.col, at.row)?;
        self.anchor = Some(TrackedAnchor {
            start,
            end,
            rectangle: false,
        });
        Ok(())
    }

    /// Move the drag endpoint to a viewport cell.
    ///
    /// With no active selection this acts as [`SelectionState::begin`], so
    /// callers need not order press/drag events strictly.
    pub fn extend(&mut self, term: &Terminal, to: CellPoint) -> Result<(), Error> {
        let Some(anchor) = self.anchor.as_mut() else {
            return self.begin(term, to);
        };
        anchor.end = term.track_point(to.col, to.row)?;
        Ok(())
    }

    /// Interpret the endpoints as a block rectangle (or back to linear).
    ///
    /// No-op without an active selection.
    pub fn set_rectangle(&mut self, rectangle: bool) {
        if let Some(anchor) = self.anchor.as_mut() {
            anchor.rectangle = rectangle;
        }
    }

    /// Drop the selection. Does not touch the terminal-owned selection —
    /// see [`SelectionState::uninstall`].
    pub fn clear(&mut self) {
        self.anchor = None;
    }

    /// Resolve the tracked endpoints to a selection snapshot.
    ///
    /// Returns `Ok(None)` without a selection, or when tracked content has
    /// left the terminal (e.g. scrolled out of the scrollback limit).
    pub fn resolve<'a>(&self, term: &'a Terminal) -> Result<Option<Selection<'a>>, Error> {
        let Some(anchor) = self.anchor.as_ref() else {
            return Ok(None);
        };
        let (Some(start), Some(end)) = (
            anchor.start.snapshot(term.inner())?,
            anchor.end.snapshot(term.inner())?,
        ) else {
            return Ok(None);
        };
        Ok(Some(Selection::new(start, end, anchor.rectangle)))
    }

    /// Copy the selection as plain text.
    ///
    /// Linear selections format through the terminal (wrapping-aware);
    /// block selections read the cell rectangle row by row, trailing blanks
    /// trimmed. Returns `Ok(None)` when nothing is selected.
    pub fn copy_text(&self, term: &Terminal) -> Result<Option<String>, Error> {
        let Some(anchor) = self.anchor.as_ref() else {
            return Ok(None);
        };
        if anchor.rectangle {
            return Ok(Some(self.copy_rectangle(term, anchor)?));
        }
        let Some(sel) = self.resolve(term)? else {
            return Ok(None);
        };
        Ok(Some(term.selection_text(&sel)?))
    }

    /// Install the selection as the terminal-owned selection so snapshots
    /// (and the grid renderer) mark its cells `selected`.
    ///
    /// Returns `Ok(None)` when nothing is selected.
    pub fn install(&self, term: &Terminal) -> Result<Option<()>, Error> {
        let Some(sel) = self.resolve(term)? else {
            return Ok(None);
        };
        term.set_selection(Some(&sel))?;
        Ok(Some(()))
    }

    /// Remove the terminal-owned selection. Keeps the app-side state — call
    /// [`SelectionState::clear`] too when the selection itself should end.
    pub fn uninstall(term: &Terminal) -> Result<(), Error> {
        term.set_selection(None)
    }

    /// Read the cell rectangle between the tracked endpoints (inclusive).
    ///
    /// Endpoints resolve in screen space so the copy works while scrolled;
    /// direction is irrelevant (corners normalize). Wide-cell spacer tails
    /// carry empty text and contribute nothing.
    fn copy_rectangle(&self, term: &Terminal, anchor: &TrackedAnchor) -> Result<String, Error> {
        let (Some(a), Some(b)) = (
            anchor.start.point(PointSpace::Screen)?,
            anchor.end.point(PointSpace::Screen)?,
        ) else {
            return Ok(String::new());
        };
        let (top, bottom) = normalise(a.y, b.y);
        let (left, right) = normalise(a.x, b.x);
        let mut out = String::new();
        for (i, row) in (top..=bottom).enumerate() {
            if i > 0 {
                out.push('\n');
            }
            let mut line = String::new();
            for col in left..=right {
                line.push_str(&term.screen_cell_text(col, row)?);
            }
            out.push_str(line.trim_end());
        }
        Ok(out)
    }
}

impl Default for SelectionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Order two bounds low-first.
fn normalise<T: Ord>(a: T, b: T) -> (T, T) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Scroll the viewport by `delta` rows (negative scrolls up into history).
///
/// Thin wrapper over [`ScrollViewport::Delta`] so callers (wheel, keyboard)
/// share one path. Alternate screen has no scrollback: the terminal clamps
/// there and the viewport stays on the active area.
pub fn scroll_lines(term: &mut Terminal, delta: isize) {
    term.scroll_viewport(ScrollViewport::Delta(delta));
}

/// Return the viewport to live output (bottom of the scrollback).
///
/// Call on input: the task contract requires the view to follow new output
/// once the user types, even when scrolled into history.
pub fn snap_to_live(term: &mut Terminal) {
    term.scroll_viewport(ScrollViewport::Bottom);
}

/// Scrollback position indicator derived from [`Scrollbar`].
///
/// `offset` is the viewport top in the same row space as `total`
/// (row 0 is the oldest scrollback line), so a scrollbar position
/// round-trips into [`ScrollViewport::Row`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollIndicator {
    /// Total scrollable rows (scrollback + viewport).
    pub total: u64,
    /// Viewport top row in the total row space.
    pub offset: u64,
    /// Viewport height in rows.
    pub len: u64,
}

impl ScrollIndicator {
    /// Capture the current indicator state. Poll once per frame or write
    /// batch and diff — the terminal sends no scroll notifications (this is
    /// what Ghostty's own renderer does).
    pub fn capture(term: &Terminal) -> Result<Self, Error> {
        let bar = term.scrollbar()?;
        Ok(Self {
            total: bar.total,
            offset: bar.offset,
            len: bar.len,
        })
    }

    /// Whether the viewport is pinned to live output.
    #[must_use]
    pub fn at_live(&self) -> bool {
        self.offset + self.len >= self.total
    }

    /// Viewport position as a 0.0 (top) .. 1.0 (live) fraction.
    ///
    /// Returns `None` when there is nothing to scroll (`total <= len`,
    /// e.g. alternate screen) so the indicator can hide itself.
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        if self.total <= self.len {
            return None;
        }
        let max_offset = (self.total - self.len) as f32;
        Some((self.offset as f32 / max_offset).clamp(0.0, 1.0))
    }

    /// Absolute row for [`ScrollViewport::Row`] that restores this position.
    #[must_use]
    pub fn row(&self) -> usize {
        self.offset as usize
    }
}

/// Current viewport coordinates of a tracked point, if visible.
///
/// Helper for tests and the future 9.3 mouse layer.
#[must_use]
pub fn viewport_coords_of(tracked: &TrackedGridRef) -> Option<PointCoordinate> {
    // `point` only fails on native errors; treat any failure as not-visible.
    tracked.point(PointSpace::Viewport).ok()?
}

#[cfg(test)]
mod tests {
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
    fn drag_copies_linear_text() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"alpha\r\nbeta");
        let mut sel = SelectionState::new();
        assert!(!sel.is_active());
        sel.begin(&term, CellPoint { col: 0, row: 0 })?;
        sel.extend(&term, CellPoint { col: 4, row: 0 })?;
        assert!(sel.is_active());
        let text = sel.copy_text(&term)?.unwrap_or_default();
        assert!(text.contains("alpha"), "drag missed line: {text:?}");
        assert!(!text.contains("beta"), "drag bled rows: {text:?}");
        Ok(())
    }

    #[test]
    fn rectangle_copies_block() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"abcdef\r\nghijkl");
        let mut sel = SelectionState::new();
        sel.begin(&term, CellPoint { col: 1, row: 0 })?;
        sel.extend(&term, CellPoint { col: 2, row: 1 })?;
        sel.set_rectangle(true);
        let text = sel.copy_text(&term)?.unwrap_or_default();
        assert_eq!(text, "bc\nhi");
        Ok(())
    }

    #[test]
    fn reversed_drag_copies_same_text() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"alpha");
        let mut fwd = SelectionState::new();
        fwd.begin(&term, CellPoint { col: 0, row: 0 })?;
        fwd.extend(&term, CellPoint { col: 4, row: 0 })?;
        let mut rev = SelectionState::new();
        rev.begin(&term, CellPoint { col: 4, row: 0 })?;
        rev.extend(&term, CellPoint { col: 0, row: 0 })?;
        assert_eq!(fwd.copy_text(&term)?, rev.copy_text(&term)?);
        Ok(())
    }

    #[test]
    fn selection_survives_output_underneath() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"alpha");
        let mut sel = SelectionState::new();
        sel.begin(&term, CellPoint { col: 0, row: 0 })?;
        sel.extend(&term, CellPoint { col: 4, row: 0 })?;
        let before = sel.copy_text(&term)?;
        // More output must neither disturb nor clear the selection.
        term.feed(b"\r\nbeta\r\ngamma");
        assert!(sel.is_active());
        assert_eq!(sel.copy_text(&term)?, before);
        Ok(())
    }

    #[test]
    fn clear_ends_selection() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"alpha");
        let mut sel = SelectionState::new();
        sel.begin(&term, CellPoint { col: 0, row: 0 })?;
        sel.extend(&term, CellPoint { col: 4, row: 0 })?;
        sel.clear();
        assert!(!sel.is_active());
        assert!(sel.copy_text(&term)?.is_none());
        assert!(sel.resolve(&term)?.is_none());
        Ok(())
    }

    #[test]
    fn install_marks_snapshot_cells_selected() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"alpha");
        let mut sel = SelectionState::new();
        sel.begin(&term, CellPoint { col: 0, row: 0 })?;
        sel.extend(&term, CellPoint { col: 4, row: 0 })?;
        assert!(sel.install(&term)?.is_some());
        let snap = term.snapshot()?;
        assert!(snap.rows_data[0].cells[0].selected);
        assert!(!snap.rows_data[0].cells[10].selected);
        SelectionState::uninstall(&term)?;
        let snap = term.snapshot()?;
        assert!(!snap.rows_data[0].cells[0].selected);
        Ok(())
    }

    #[test]
    fn word_select_copies_word() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"hello world");
        let word = term.select_word_text(1, 0)?.unwrap_or_default();
        assert_eq!(word, "hello");
        Ok(())
    }

    #[test]
    fn scrollback_navigates_and_snaps_to_live() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        for i in 0..10 {
            term.feed(format!("line{i}\r\n").as_bytes());
        }
        assert!(term.scrollback_rows()? > 0);
        let live = ScrollIndicator::capture(&term)?;
        assert!(live.at_live());
        assert_eq!(live.fraction(), Some(1.0));

        scroll_lines(&mut term, -2);
        let away = ScrollIndicator::capture(&term)?;
        assert!(!away.at_live());
        let frac = away.fraction().unwrap_or(f32::NAN);
        assert!((0.0..1.0).contains(&frac), "fraction {frac} not mid-range");

        snap_to_live(&mut term);
        let back = ScrollIndicator::capture(&term)?;
        assert!(back.at_live());
        assert_eq!(back.fraction(), Some(1.0));
        // Row space round-trips: restoring the captured row keeps position.
        term.scroll_viewport(ScrollViewport::Row(away.row()));
        assert_eq!(ScrollIndicator::capture(&term)?, away);
        Ok(())
    }
}
