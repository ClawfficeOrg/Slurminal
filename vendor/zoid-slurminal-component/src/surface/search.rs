//! Incremental search over the scrollback buffer (task 9.2.4).
//!
//! The bindings expose no native search, so matching runs over screen-space
//! row text (full screen including scrollback, row 0 oldest). Matches carry
//! screen-space rows plus cell columns for in-grid highlight; [`locate`]
//! maps a match into viewport rows for the visible frame, or `None` when the
//! match is scrolled out of view. The 9.3 GPUI layer owns the highlight
//! rendering — this module only finds and cycles.
//!
//! Column mapping is per-character, not per-byte: each character of a row
//! remembers the cell column that produced it, so wide-cell spacer tails
//! (empty text) never shift highlights. Case-insensitive search lowercases
//! per character and carries each folded character's original column, so
//! folding never misaligns a match either.
//!
//! Plain Rust, no GPUI dependency (same rule as the rest of `surface/`).
//! Full-buffer scan is `O(rows * cols)` FFI reads per keystroke; fine for
//! correctness now, but incremental caching belongs in a later pass —
//! `SearchState::update` re-scans from scratch by design, documented here
//! so the future optimisation has a place to land.

use crate::terminal::{Error, Scrollbar, Terminal};

/// Search matching options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Fold case before matching. Default false (case-insensitive), the
    /// usual incremental-search behaviour.
    pub case_sensitive: bool,
}

/// One match: screen-space row plus cell columns for highlight.
///
/// `col_end` is exclusive. Both columns address cells, so a match covering
/// a wide glyph spans its full width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    /// Screen-space row (0 is the oldest scrollback line).
    pub screen_row: u32,
    /// First cell column of the match.
    pub col_start: u16,
    /// One past the last cell column of the match.
    pub col_end: u16,
}

/// Find all non-overlapping matches of `needle` in row-major order.
///
/// Returns an empty vector for an empty needle (never "match everything").
/// Reads screen-space rows, so history matches are found, not just the
/// viewport.
pub fn find_matches(
    term: &Terminal,
    needle: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchMatch>, Error> {
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let needle_key: Vec<char> = key_chars(needle, options.case_sensitive);
    let cols = term.cols()?;
    let total = term.scrollbar()?.total.min(u32::MAX as u64) as u32;
    let mut matches = Vec::new();
    for screen_row in 0..total {
        let (text, columns) = screen_row_key(term, cols, screen_row, options.case_sensitive)?;
        matches.extend(row_matches(screen_row, &text, &columns, &needle_key));
    }
    Ok(matches)
}

/// Incremental search state: current query, its matches, and the cursor.
///
/// `update` re-runs the scan and resets to the first match. `next` /
/// `previous` cycle row-major with wraparound; empty match lists stay at
/// `None` rather than panicking.
pub struct SearchState {
    needle: String,
    options: SearchOptions,
    matches: Vec<SearchMatch>,
    current: Option<usize>,
}

impl SearchState {
    /// Empty state (no query, no matches).
    #[must_use]
    pub fn new() -> Self {
        Self {
            needle: String::new(),
            options: SearchOptions::default(),
            matches: Vec::new(),
            current: None,
        }
    }

    /// Re-run the scan for a new query string, resetting to the first match.
    pub fn update(
        &mut self,
        term: &Terminal,
        needle: &str,
        options: SearchOptions,
    ) -> Result<(), Error> {
        self.needle = needle.to_string();
        self.options = options;
        self.matches = find_matches(term, needle, &options)?;
        self.current = if self.matches.is_empty() {
            None
        } else {
            Some(0)
        };
        Ok(())
    }

    /// Current query string.
    #[must_use]
    pub fn needle(&self) -> &str {
        &self.needle
    }

    /// All matches in row-major order.
    #[must_use]
    pub fn matches(&self) -> &[SearchMatch] {
        &self.matches
    }

    /// The current match, if any.
    #[must_use]
    pub fn current(&self) -> Option<&SearchMatch> {
        self.current.and_then(|i| self.matches.get(i))
    }

    /// Advance to the next match (wraps around). No-op when empty.
    pub fn next(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        let i = self.current.unwrap_or(0);
        self.current = Some((i + 1) % self.matches.len());
    }

    /// Step back to the previous match (wraps around). No-op when empty.
    pub fn previous(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        let i = self.current.unwrap_or(0);
        self.current = Some((i + self.matches.len() - 1) % self.matches.len());
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a match to the viewport row it is visible on, if any.
///
/// Screen rows address from the oldest scrollback line while the viewport
/// sits at `bar.offset`, so `screen_row - offset` is the viewport row when
/// it falls inside `[offset, offset + len)`. Returns `None` for matches
/// scrolled out of view.
#[must_use]
pub fn locate(m: &SearchMatch, bar: &Scrollbar) -> Option<u16> {
    let row = m.screen_row as u64;
    if row < bar.offset || row >= bar.offset + bar.len {
        return None;
    }
    u16::try_from(row - bar.offset).ok()
}

/// Fold a query string to match-key characters.
fn key_chars(s: &str, case_sensitive: bool) -> Vec<char> {
    if case_sensitive {
        s.chars().collect()
    } else {
        s.chars().flat_map(|c| c.to_lowercase()).collect()
    }
}

/// Read one screen-space row as match-key characters plus the cell column
/// that produced each character.
fn screen_row_key(
    term: &Terminal,
    cols: u16,
    screen_row: u32,
    case_sensitive: bool,
) -> Result<(Vec<char>, Vec<u16>), Error> {
    let mut text = Vec::new();
    let mut columns = Vec::new();
    for col in 0..cols {
        let cell = term.screen_cell_text(col, screen_row)?;
        for c in cell.chars() {
            if case_sensitive {
                text.push(c);
                columns.push(col);
            } else {
                // Each folded char inherits its source cell: folding never
                // shifts highlight columns, even when it changes char count.
                for folded in c.to_lowercase() {
                    text.push(folded);
                    columns.push(col);
                }
            }
        }
    }
    Ok((text, columns))
}

/// Non-overlapping left-to-right matches of `needle` in one keyed row.
fn row_matches(
    screen_row: u32,
    text: &[char],
    columns: &[u16],
    needle: &[char],
) -> Vec<SearchMatch> {
    let mut out = Vec::new();
    if needle.is_empty() || text.len() < needle.len() {
        return out;
    }
    let mut i = 0;
    while i + needle.len() <= text.len() {
        if text[i..i + needle.len()] == *needle {
            out.push(SearchMatch {
                screen_row,
                col_start: columns[i],
                // Exclusive end: column after the last matched cell.
                col_end: columns[i + needle.len() - 1] + 1,
            });
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
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

    fn feed_lines(term: &mut Terminal, n: u32) -> Result<(), Error> {
        for i in 0..n {
            term.feed(format!("line{i}\r\n").as_bytes());
        }
        Ok(())
    }

    #[test]
    fn finds_matches_in_row_major_order() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"foo bar\r\nbaz foo");
        let matches = find_matches(&term, "foo", &SearchOptions::default())?;
        assert_eq!(matches.len(), 2);
        assert!(matches[0].screen_row < matches[1].screen_row);
        assert_eq!((matches[0].col_start, matches[0].col_end), (0, 3));
        assert_eq!((matches[1].col_start, matches[1].col_end), (4, 7));
        Ok(())
    }

    #[test]
    fn default_search_is_case_insensitive() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"Hello World");
        assert_eq!(
            find_matches(&term, "hello", &SearchOptions::default())?.len(),
            1
        );
        assert!(
            find_matches(
                &term,
                "hello",
                &SearchOptions {
                    case_sensitive: true
                }
            )?
            .is_empty()
        );
        Ok(())
    }

    #[test]
    fn empty_needle_matches_nothing() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"alpha");
        assert!(find_matches(&term, "", &SearchOptions::default())?.is_empty());
        Ok(())
    }

    #[test]
    fn state_cycles_with_wraparound() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"foo\r\nfoo");
        let mut state = SearchState::new();
        state.update(&term, "foo", SearchOptions::default())?;
        assert_eq!(state.matches().len(), 2);
        let first = state.current().copied().unwrap_or(SearchMatch {
            screen_row: u32::MAX,
            col_start: u16::MAX,
            col_end: u16::MAX,
        });
        state.next();
        let second = state.current().copied().unwrap_or(SearchMatch {
            screen_row: u32::MAX,
            col_start: u16::MAX,
            col_end: u16::MAX,
        });
        assert_ne!(first, second);
        state.next();
        assert_eq!(state.current(), Some(&first));
        state.previous();
        assert_eq!(state.current(), Some(&second));
        Ok(())
    }

    #[test]
    fn update_resets_to_first_match() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        term.feed(b"foo\r\nfoo");
        let mut state = SearchState::new();
        state.update(&term, "foo", SearchOptions::default())?;
        state.next();
        state.update(&term, "foo", SearchOptions::default())?;
        assert_eq!(state.current(), state.matches().first());
        state.update(&term, "zzz", SearchOptions::default())?;
        assert!(state.matches().is_empty());
        assert_eq!(state.current(), None);
        // Cycling an empty state is a no-op, never a panic.
        state.next();
        state.previous();
        Ok(())
    }

    #[test]
    fn locate_maps_history_and_viewport() -> Result<(), Error> {
        let mut term = test_terminal(20, 4)?;
        feed_lines(&mut term, 10)?;
        let matches = find_matches(&term, "line", &SearchOptions::default())?;
        assert!(!matches.is_empty());
        // At live output the oldest history matches are out of view while
        // the newest sit in the viewport.
        let bar = term.scrollbar()?;
        assert_eq!(locate(&matches[0], &bar), None);
        let last = matches.last().copied().unwrap_or(matches[0]);
        assert!(locate(&last, &bar).is_some());
        // Scrolled to the top, the oldest match is on the first viewport row.
        term.scroll_viewport(crate::terminal::ScrollViewport::Top);
        let bar = term.scrollbar()?;
        assert_eq!(locate(&matches[0], &bar), Some(0));
        assert_eq!(locate(&last, &bar), None);
        Ok(())
    }
}
