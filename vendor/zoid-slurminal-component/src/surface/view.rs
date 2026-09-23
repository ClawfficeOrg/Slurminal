//! Retained GPUI view owning one terminal (task 9.3.1).
//!
//! This is the type ZML `Terminal` nodes map to. The codegen emitter holds
//! it in `ZmlEntities` (`Entity<TerminalView>`) and the layout renders the
//! handle: the view owns the [`Terminal`] (which must outlive frames —
//! building one per render would reset scrollback, selection, and modes),
//! snapshots it every frame, and renders the [`TerminalGrid`].
//!
//! Construction is infallible by design: generated code cannot propagate a
//! `Result`, so a terminal that fails to initialise leaves a not-live view
//! ([`TerminalView::is_live`]) rendering empty rather than panicking. The
//! app reads the [`TerminalView::working_directory`] hint when spawning the
//! shell.
//!
//! PTY wiring (task 9.3.5) is bytes in, bytes out, with the PTY itself
//! staying app-owned:
//!
//! - **Out of the PTY:** the app hands child output to
//!   [`TerminalView::feed`] (not `terminal_mut().feed`), which parses it,
//!   repaints, and forwards any query responses the VT core produced.
//! - **Into the PTY:** the view emits [`TerminalEvent::Input`] for encoded
//!   key presses (while focused) and for those query responses. The app
//!   subscribes and writes the bytes to its PTY. Responses matter: ConPTY
//!   asks for the cursor position (`CSI 6 n`) at startup and waits for the
//!   answer before the shell draws anything.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, ParentElement as _, Render, SharedString, Styled as _, Window,
    div, px,
};
use gpui_kit::component::ActiveTheme as _;

use super::keys::{KeyEncoder, key_press_from_keystroke};
use super::{SurfaceConfig, TerminalGrid, TerminalPalette, measure_cell_metrics};
use crate::terminal::{Terminal, TerminalConfig};

/// Events a [`TerminalView`] emits to its host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Bytes for the PTY: encoded key presses and VT query responses.
    /// Write them to the child verbatim (`PtySession::write_all`).
    Input(Vec<u8>),
}

/// Default terminal size in cells before the viewport drives a resize.
const DEFAULT_COLS: u16 = 80;
/// Default terminal size in cells before the viewport drives a resize.
const DEFAULT_ROWS: u16 = 24;

/// A live terminal embedded in a GPUI view hierarchy.
///
/// Owns the emulator core plus the surface configuration. Colors resolve
/// from the [`PaletteSource`][super::PaletteSource] every render: a custom
/// palette renders as stored, the theme source re-derives from
/// `cx.theme()` and re-applies on change — no recreation, matching the
/// 9.2.5 contract.
pub struct TerminalView {
    terminal: Option<Terminal>,
    config: SurfaceConfig,
    scrollback_limit: usize,
    working_directory: Option<String>,
    applied: Option<TerminalPalette>,
    /// Query responses the VT core wrote back, pending emission.
    responses: Rc<RefCell<Vec<u8>>>,
    /// Created on first render or [`TerminalView::focus`]: construction
    /// stays context-free so codegen can keep calling `new()`.
    focus_handle: Option<FocusHandle>,
}

impl TerminalView {
    /// Build a view with default configuration.
    ///
    /// Codegen appends `.with_*` builders for the ZML props; see
    /// `entity_prop_sink` in `zml_codegen.rs`.
    #[must_use]
    pub fn new() -> Self {
        let responses = Rc::new(RefCell::new(Vec::new()));
        let terminal = Terminal::new(TerminalConfig {
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            ..TerminalConfig::default()
        })
        .ok()
        .and_then(|mut term| {
            let sink = Rc::clone(&responses);
            term.on_pty_write(move |_, bytes| sink.borrow_mut().extend_from_slice(bytes))
                .ok()
                .map(|()| term)
        });
        Self {
            terminal,
            config: SurfaceConfig::default(),
            scrollback_limit: TerminalConfig::default().max_scrollback,
            working_directory: None,
            applied: None,
            responses,
            focus_handle: None,
        }
    }

    /// Feed PTY output into the terminal and repaint.
    ///
    /// Prefer this over `terminal_mut().feed`: it also emits any query
    /// responses the output provoked as [`TerminalEvent::Input`], which
    /// the host must write back to the PTY.
    pub fn feed(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        if let Some(term) = self.terminal.as_mut() {
            term.feed(bytes);
        }
        let responses = std::mem::take(&mut *self.responses.borrow_mut());
        if !responses.is_empty() {
            cx.emit(TerminalEvent::Input(responses));
        }
        cx.notify();
    }

    /// Move keyboard focus to the terminal so key presses reach the PTY.
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.ensure_focus_handle(cx);
        window.focus(&handle, cx);
    }

    /// Whether the terminal currently holds keyboard focus.
    #[must_use]
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle
            .as_ref()
            .is_some_and(|handle| handle.is_focused(window))
    }

    fn ensure_focus_handle(&mut self, cx: &mut Context<Self>) -> FocusHandle {
        self.focus_handle
            .get_or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// Encode a key press against the live terminal modes and emit it.
    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(term) = self.terminal.as_ref() else {
            return;
        };
        let press = key_press_from_keystroke(&event.keystroke);
        // Encoder per press: it is a cheap FFI allocation, and syncing from
        // the terminal each time means mode changes can never go stale.
        let Ok(mut encoder) = KeyEncoder::new() else {
            return;
        };
        let Ok(bytes) = encoder.sync_from(term).encode_press(&press) else {
            return;
        };
        if !bytes.is_empty() {
            cx.emit(TerminalEvent::Input(bytes));
            cx.stop_propagation();
        }
    }

    /// Override the terminal font family (ZML `font_family`).
    #[must_use]
    pub fn with_font_family(mut self, family: impl Into<SharedString>) -> Self {
        self.config.font_family = family.into();
        self
    }

    /// Override the terminal font size in pixels (ZML `font_size`).
    #[must_use]
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.config.font_size = px(size);
        self
    }

    /// Cap scrollback rows (ZML `scrollback_limit`).
    ///
    /// Applied to the live terminal immediately; a terminal that rejects
    /// the limit leaves the view not-live rather than half-configured.
    #[must_use]
    pub fn with_scrollback_limit(mut self, limit: usize) -> Self {
        self.scrollback_limit = limit;
        if let Some(term) = self.terminal.as_mut()
            && term.set_scrollback_max_lines(Some(limit)).is_err()
        {
            self.terminal = None;
        }
        self
    }

    /// Initial working-directory hint for the app's PTY spawn (ZML
    /// `working_directory`). The component never spawns processes itself.
    #[must_use]
    pub fn with_working_directory(mut self, dir: impl Into<String>) -> Self {
        self.working_directory = Some(dir.into());
        self
    }

    /// Standalone color scheme, ignoring the app theme (ZML `palette`).
    ///
    /// Applied to the live terminal immediately; same not-live policy as
    /// [`TerminalView::with_scrollback_limit`].
    #[must_use]
    pub fn with_palette(mut self, palette: TerminalPalette) -> Self {
        self.config.palette = super::PaletteSource::Custom(palette);
        if let Some(term) = self.terminal.as_mut()
            && palette.apply_to(term).is_err()
        {
            self.terminal = None;
        }
        self
    }

    /// Whether the terminal initialised and holds every applied setting.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.terminal.is_some()
    }

    /// Borrow the emulator core (feed output, read state).
    #[must_use]
    pub fn terminal(&self) -> Option<&Terminal> {
        self.terminal.as_ref()
    }

    /// Mutably borrow the emulator core (resize, modes, effect callbacks).
    ///
    /// Feed PTY output through [`TerminalView::feed`] instead, so query
    /// responses reach the PTY. Registering `on_pty_write` here replaces
    /// the view's own handler and silences those responses.
    #[must_use]
    pub fn terminal_mut(&mut self) -> Option<&mut Terminal> {
        self.terminal.as_mut()
    }

    /// The `working_directory` hint, if the ZML node set one.
    #[must_use]
    pub fn working_directory(&self) -> Option<&str> {
        self.working_directory.as_deref()
    }

    /// The surface configuration (font, palette source).
    #[must_use]
    pub fn config(&self) -> &SurfaceConfig {
        &self.config
    }
}

impl Default for TerminalView {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter<TerminalEvent> for TerminalView {}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.ensure_focus_handle(cx);
        let palette = match self.config.palette {
            super::PaletteSource::Custom(palette) => palette,
            super::PaletteSource::Theme => TerminalPalette::from_theme_colors(&cx.theme().colors),
        };
        // Re-apply on change only: four FFI setter calls per theme switch,
        // not per frame.
        if self.applied != Some(palette)
            && let Some(term) = self.terminal.as_mut()
            && palette.apply_to(term).is_ok()
        {
            self.applied = Some(palette);
        }
        let mut children = Vec::new();
        if let Some(term) = self.terminal.as_ref()
            && let Ok(snap) = term.snapshot()
        {
            let metrics = measure_cell_metrics(cx, &self.config);
            children.push(TerminalGrid::from_themed_snapshot(
                &snap,
                metrics,
                &self.config,
                &palette,
            ));
        }
        div()
            .key_context("Terminal")
            .track_focus(&focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _: &MouseDownEvent, window, cx| view.focus(window, cx)),
            )
            .size_full()
            .children(children)
    }
}

#[cfg(test)]
mod tests {
    use gpui::AppContext as _;
    use gpui_kit::component::ThemeColor;

    use super::*;

    #[test]
    fn defaults_are_live_with_no_working_directory() {
        let view = TerminalView::new();
        assert!(view.is_live());
        assert_eq!(view.working_directory(), None);
        assert!(view.terminal().is_some());
    }

    #[test]
    fn builders_store_config_and_apply_palette() {
        let palette = TerminalPalette::from_theme_colors(&ThemeColor::dark());
        let view = TerminalView::new()
            .with_font_family("Consolas")
            .with_font_size(16.0)
            .with_scrollback_limit(500)
            .with_working_directory("/tmp")
            .with_palette(palette);
        assert!(view.is_live());
        assert_eq!(view.config().font_family.as_ref(), "Consolas");
        assert_eq!(view.working_directory(), Some("/tmp"));
        let fg = view
            .terminal()
            .and_then(|t| t.fg_color().unwrap_or(None))
            .unwrap_or_default();
        assert_eq!(fg, palette.foreground);
    }

    /// Collects every `TerminalEvent::Input` a view emits.
    fn capture_input(
        cx: &mut gpui::TestAppContext,
    ) -> (gpui::Entity<TerminalView>, Rc<RefCell<Vec<u8>>>) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let view = cx.update(|cx| {
            let view = cx.new(|_| TerminalView::new());
            cx.subscribe(&view, move |_, event: &TerminalEvent, _| {
                let TerminalEvent::Input(bytes) = event;
                sink.borrow_mut().extend_from_slice(bytes);
            })
            .detach();
            view
        });
        (view, seen)
    }

    #[gpui::test]
    fn feed_forwards_query_responses_as_input(cx: &mut gpui::TestAppContext) {
        let (view, seen) = capture_input(cx);
        // DSR 6 (cursor position report): ConPTY sends this at startup and
        // blocks until the terminal answers.
        view.update(cx, |view, cx| view.feed(b"ab[6n", cx));
        assert_eq!(seen.borrow().as_slice(), b"[1;3R");
        // Plain output provokes no response.
        seen.borrow_mut().clear();
        view.update(cx, |view, cx| view.feed(b"plain", cx));
        assert!(seen.borrow().is_empty());
    }

    #[gpui::test]
    fn feed_updates_the_grid(cx: &mut gpui::TestAppContext) {
        let (view, _seen) = capture_input(cx);
        view.update(cx, |view, cx| view.feed(b"hello", cx));
        let text = view.read_with(cx, |view, _| {
            view.terminal()
                .and_then(|t| t.row_text(0).ok())
                .unwrap_or_default()
        });
        assert_eq!(text, "hello");
    }

    #[test]
    fn terminal_accepts_feeds_through_the_view() -> Result<(), crate::terminal::Error> {
        let mut view = TerminalView::new();
        assert!(view.is_live(), "test terminal must be live");
        if let Some(term) = view.terminal_mut() {
            term.feed(b"hi");
        }
        let text = view
            .terminal()
            .map(|t| t.row_text(0))
            .transpose()?
            .unwrap_or_default();
        assert_eq!(text, "hi");
        Ok(())
    }
}
