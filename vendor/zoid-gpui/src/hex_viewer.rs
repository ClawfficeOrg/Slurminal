//! `HexViewer` component for generated GPUI starters.
//!
//! Provides [`HexViewer`] — a stateful, read-only GPUI view that renders
//! binary data as a classic hex dump with three aligned columns:
//!
//! - **Offset**: hex row address, zero-padded to eight characters.
//! - **Hex bytes**: one `xx` cell per byte, split into two equal halves for
//!   readability (classic hex-editor layout).
//! - **ASCII**: one character per byte; non-printable bytes (`< 0x20` or
//!   `>= 0x7F`) render as `.`.
//!
//! [`HexViewer`] uses [`gpui::uniform_list`] to virtualize rendering — only
//! the rows currently visible in the scroll area are constructed, so very
//! large `Arc<[u8]>` slices (hundreds of megabytes) do not cause layout
//! hangs or excessive allocations.
//!
//! Clicking a hex-byte cell **or** its corresponding ASCII cell selects that
//! byte offset and highlights it in both columns.  An optional
//! `on_select(offset: usize)` callback is fired on every click.
//!
//! The viewer is **read-only**: no editing is supported.
//!
//! # Example
//!
//! ```rust,ignore
//! let viewer = cx.new(|cx| {
//!     HexViewer::new(cx)
//!         .with_data(Arc::from(&b"Hello, GPUI!"[..]))
//!         .with_bytes_per_row(16)
//!         .with_visible_height(300.0)
//!         .on_select(|offset, _w, _cx| println!("selected byte 0x{offset:x}"))
//!         .dark(true)
//! });
//! ```
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::{rc::Rc, sync::Arc};

use gpui::{
    App, Context, ElementId, IntoElement, Render, SharedString, Window, div, prelude::*, px,
    uniform_list,
};

use crate::ui_tokens::{RadiusScale, TypographyScale};
use gpui_kit::component::{Theme, ThemeColor};

// ── handler type ──────────────────────────────────────────────────────────────

/// Callback fired when the user clicks a byte cell, selecting it.
///
/// Receives the byte's zero-based offset into the data slice, plus mutable
/// window and app context references.  Only invoked from UI click events
/// where a [`Window`] is available.
pub type HexSelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

// ── HexViewerColors ───────────────────────────────────────────────────────────

/// Token-resolved display colors for a [`HexViewer`].
///
/// Exposed so unit tests can verify token bindings without constructing a
/// full GPUI render context.
#[derive(Clone, Copy, Debug)]
pub struct HexViewerColors {
    /// Background of the entire viewer container.
    pub container_bg: gpui::Hsla,
    /// Border color of the viewer container.
    pub container_border: gpui::Hsla,
    /// Default row background (even rows).
    pub row_bg: gpui::Hsla,
    /// Alternate row background (odd rows).
    pub row_alt_bg: gpui::Hsla,
    /// Text color for the offset column.
    pub offset_text: gpui::Hsla,
    /// Text color for hex byte values.
    pub hex_text: gpui::Hsla,
    /// Text color for printable ASCII characters.
    pub ascii_text: gpui::Hsla,
    /// Text color for non-printable ASCII placeholders (`.`).
    pub non_printable_text: gpui::Hsla,
    /// Background of a selected byte cell (both hex and ASCII columns).
    pub selected_bg: gpui::Hsla,
    /// Foreground text of a selected byte cell.
    pub selected_text: gpui::Hsla,
    /// Color of the `|` column separator.
    pub separator: gpui::Hsla,
}

impl HexViewerColors {
    /// Resolves hex viewer colors from the given `tokens`.
    #[must_use]
    pub fn resolve(theme: &Theme) -> Self {
        Self {
            container_bg: theme.secondary,
            container_border: theme.border,
            row_bg: theme.secondary,
            row_alt_bg: theme.secondary,
            offset_text: theme.muted_foreground,
            hex_text: theme.foreground,
            ascii_text: theme.foreground,
            non_printable_text: theme.muted_foreground,
            selected_bg: theme.accent,
            selected_text: theme.accent_foreground,
            separator: theme.border,
        }
    }
}

// ── HexViewer ─────────────────────────────────────────────────────────────────

/// Read-only, virtualized hex dump viewer GPUI view.
///
/// Renders `Arc<[u8]>` as offset | hex bytes | ASCII with per-byte click
/// selection highlighted in both columns.  Only visible rows are rendered
/// via [`gpui::uniform_list`] regardless of data size.
///
/// See the [module-level documentation](self) for a complete usage example.
pub struct HexViewer {
    /// The binary data slice to display.  Shared; never copied per-frame.
    data: Arc<[u8]>,
    /// Number of bytes displayed per row (default: 16, clamped to `[1, 64]`).
    bytes_per_row: usize,
    /// Currently selected byte offset, or `None` for no selection.
    selected: Option<usize>,
    /// Optional callback invoked when the user selects a byte via click.
    on_select: Option<HexSelectHandler>,
    /// Render in dark-theme colors when `true`.
    dark: bool,
    /// Fixed row height in logical pixels.
    row_height: f32,
    /// Visible body height in logical pixels.
    visible_height: f32,
}

impl HexViewer {
    // ── construction ─────────────────────────────────────────────────────────

    /// Creates an empty viewer with default settings.
    ///
    /// Chain builder methods before passing the value to `cx.new`.
    #[must_use]
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            data: Arc::from([]),
            bytes_per_row: 16,
            selected: None,
            on_select: None,
            dark: true,
            row_height: 20.0,
            visible_height: 200.0,
        }
    }

    /// Replaces the binary data to display.
    ///
    /// The `Arc<[u8]>` is stored as-is; only its reference count is
    /// incremented.  The data is never copied during rendering.
    #[must_use]
    pub fn with_data(mut self, data: Arc<[u8]>) -> Self {
        self.data = data;
        self
    }

    /// Sets the number of bytes displayed per row, clamped to `[1, 64]`.
    #[must_use]
    pub fn with_bytes_per_row(mut self, n: usize) -> Self {
        self.bytes_per_row = n.clamp(1, 64);
        self
    }

    /// Sets the visible body height in logical pixels (minimum 1.0).
    #[must_use]
    pub fn with_visible_height(mut self, h: f32) -> Self {
        self.visible_height = h.max(1.0);
        self
    }

    /// Selects the light (`false`) or dark (`true`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Registers a callback invoked when the user clicks a byte cell.
    ///
    /// The callback receives the zero-based byte offset and mutable window /
    /// app context references.  Only fired from UI-driven click events.
    #[must_use]
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    // ── runtime mutators ─────────────────────────────────────────────────────

    /// Switches the dark/light theme at runtime and schedules a re-render.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }

    /// Sets the selected byte offset and schedules a re-render.
    ///
    /// Passing `None` clears the selection.  Offsets beyond the data length
    /// are silently ignored (selection stays `None`).
    ///
    /// The `on_select` callback is **not** fired from this path — use it in
    /// headless tests to drive selection state without needing a [`Window`].
    pub fn apply_select(&mut self, offset: Option<usize>, cx: &mut Context<Self>) {
        self.selected = offset.filter(|&o| o < self.data.len());
        cx.notify();
    }

    // ── accessors ────────────────────────────────────────────────────────────

    /// Returns the length of the data slice in bytes.
    #[must_use]
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Returns the number of rows required to display the full data slice.
    ///
    /// Returns `0` for empty data or when `bytes_per_row` is zero.
    #[must_use]
    pub fn row_count(&self) -> usize {
        if self.data.is_empty() || self.bytes_per_row == 0 {
            return 0;
        }
        (self.data.len()).div_ceil(self.bytes_per_row)
    }

    /// Returns the configured bytes-per-row value.
    #[must_use]
    pub fn bytes_per_row(&self) -> usize {
        self.bytes_per_row
    }

    /// Returns the currently selected byte offset, or `None` if nothing is
    /// selected.
    #[must_use]
    pub fn selected_offset(&self) -> Option<usize> {
        self.selected
    }

    /// Returns `true` if an `on_select` callback is registered.
    #[must_use]
    pub fn has_on_select(&self) -> bool {
        self.on_select.is_some()
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for HexViewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = HexViewerColors::resolve(&theme);

        let row_count = self.row_count();
        let row_height = self.row_height;
        let visible_height = self.visible_height;
        let bytes_per_row = self.bytes_per_row;
        let selected = self.selected;

        // Capture Arc<[u8]> by clone (cheap: bumps refcount only).
        let data = Arc::clone(&self.data);
        let on_select = self.on_select.clone();
        let entity = cx.entity();

        // ── Layout constants (logical pixels) ────────────────────────────────

        // Offset column: "00000000" + small padding on each side.
        let offset_col_w = 76.0_f32;
        // Each hex byte cell: "xx" rendered in a fixed-width div.
        let byte_cell_w = 20.0_f32;
        // Normal gap between consecutive hex byte cells.
        let byte_cell_gap = 2.0_f32;
        // Extra gap that separates the first and second half of bytes.
        let group_extra_gap = 6.0_f32;
        // Separator column between hex and ASCII.
        let separator_w = 12.0_f32;
        // Each ASCII character cell.
        let ascii_cell_w = 9.0_f32;

        let typography_sm = TypographyScale::default().sm;

        // Half-row index at which the group separator is inserted.
        let half = bytes_per_row / 2;

        let body = uniform_list("hex-viewer-rows", row_count, move |range, _window, _app| {
            range
                .map(|row_idx| {
                    let row_offset = row_idx * bytes_per_row;
                    let row_end = (row_offset + bytes_per_row).min(data.len());
                    let row_bytes = &data[row_offset..row_end];
                    let row_len = row_bytes.len();

                    let bg = if row_idx % 2 == 0 {
                        colors.row_bg
                    } else {
                        colors.row_alt_bg
                    };

                    // ── Offset cell ──────────────────────────────────────

                    let offset_label = SharedString::from(format!("{row_offset:08x}"));
                    let offset_cell = div()
                        .w(px(offset_col_w))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .h(px(row_height))
                        .px(px(4.0))
                        .text_size(px(typography_sm))
                        .text_color(colors.offset_text)
                        .child(offset_label);

                    // ── Hex bytes ────────────────────────────────────────

                    // Pre-allocate: bytes + gaps + optional group separator.
                    let mut hex_children: Vec<gpui::AnyElement> =
                        Vec::with_capacity(bytes_per_row * 2 + 1);

                    // Chain real bytes (Some) followed by padding sentinels (None)
                    // so the loop variable i is always the byte index within the row.
                    let byte_slots =
                        row_bytes
                            .iter()
                            .copied()
                            .map(Some)
                            .chain(std::iter::repeat_n(
                                None,
                                bytes_per_row.saturating_sub(row_len),
                            ));

                    for (i, opt_byte) in byte_slots.enumerate() {
                        // Insert gap before every byte except the first.
                        if i > 0 {
                            let gap_w = if i == half {
                                group_extra_gap
                            } else {
                                byte_cell_gap
                            };
                            hex_children.push(
                                div()
                                    .w(px(gap_w))
                                    .flex_shrink_0()
                                    .h(px(row_height))
                                    .into_any_element(),
                            );
                        }

                        if let Some(byte_val) = opt_byte {
                            // Real byte cell — clickable.
                            let byte_offset = row_offset + i;
                            let is_selected = selected == Some(byte_offset);

                            let cell_bg = if is_selected { colors.selected_bg } else { bg };
                            let cell_fg = if is_selected {
                                colors.selected_text
                            } else {
                                colors.hex_text
                            };

                            let hex_str = SharedString::from(format!("{byte_val:02x}"));
                            let entity_hex = entity.clone();
                            let on_select_hex = on_select.clone();

                            hex_children.push(
                                div()
                                    .id(ElementId::Integer(byte_offset as u64 * 2))
                                    .w(px(byte_cell_w))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .h(px(row_height))
                                    .bg(cell_bg)
                                    .text_size(px(typography_sm))
                                    .text_color(cell_fg)
                                    .cursor_pointer()
                                    .child(hex_str)
                                    .on_click(move |_ev, window, app| {
                                        entity_hex.update(app, |v, cx| {
                                            v.selected = Some(byte_offset);
                                            cx.notify();
                                        });
                                        if let Some(ref cb) = on_select_hex {
                                            cb(byte_offset, window, app);
                                        }
                                    })
                                    .into_any_element(),
                            );
                        } else {
                            // Padding cell for incomplete last row.
                            hex_children.push(
                                div()
                                    .w(px(byte_cell_w))
                                    .flex_shrink_0()
                                    .h(px(row_height))
                                    .into_any_element(),
                            );
                        }
                    }

                    let hex_col = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .flex_shrink_0()
                        .h(px(row_height))
                        .children(hex_children);

                    // ── Column separator ─────────────────────────────────

                    let sep = div()
                        .w(px(separator_w))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(row_height))
                        .text_size(px(typography_sm))
                        .text_color(colors.separator)
                        .child(SharedString::from("|"));

                    // ── ASCII column ─────────────────────────────────────

                    let mut ascii_children: Vec<gpui::AnyElement> = Vec::with_capacity(row_len);

                    for (i, &byte_val) in row_bytes.iter().enumerate() {
                        let byte_offset = row_offset + i;
                        let is_selected = selected == Some(byte_offset);

                        let cell_bg = if is_selected { colors.selected_bg } else { bg };
                        let printable = (0x20..0x7f).contains(&byte_val);
                        let ch = if printable {
                            SharedString::from(char::from(byte_val).to_string())
                        } else {
                            SharedString::from(".")
                        };
                        let cell_fg = if is_selected {
                            colors.selected_text
                        } else if printable {
                            colors.ascii_text
                        } else {
                            colors.non_printable_text
                        };

                        let entity_ascii = entity.clone();
                        let on_select_ascii = on_select.clone();

                        ascii_children.push(
                            div()
                                .id(ElementId::Integer(byte_offset as u64 * 2 + 1))
                                .w(px(ascii_cell_w))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(row_height))
                                .bg(cell_bg)
                                .text_size(px(typography_sm))
                                .text_color(cell_fg)
                                .cursor_pointer()
                                .child(ch)
                                .on_click(move |_ev, window, app| {
                                    entity_ascii.update(app, |v, cx| {
                                        v.selected = Some(byte_offset);
                                        cx.notify();
                                    });
                                    if let Some(ref cb) = on_select_ascii {
                                        cb(byte_offset, window, app);
                                    }
                                })
                                .into_any_element(),
                        );
                    }

                    let ascii_col = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .flex_shrink_0()
                        .h(px(row_height))
                        .children(ascii_children);

                    // ── Full row ─────────────────────────────────────────

                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(px(row_height))
                        .bg(bg)
                        .child(offset_cell)
                        .child(hex_col)
                        .child(sep)
                        .child(ascii_col)
                        .into_any_element()
                })
                .collect()
        })
        .h(px(visible_height));

        div()
            .id("hex-viewer-root")
            .flex()
            .flex_col()
            .bg(colors.container_bg)
            .border_1()
            .border_color(colors.container_border)
            .rounded(px(RadiusScale::default().sm))
            .overflow_hidden()
            .child(body)
    }
}
