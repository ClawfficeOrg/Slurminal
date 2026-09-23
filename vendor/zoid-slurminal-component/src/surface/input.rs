//! Terminal input path: IME, mouse, paste, clipboard (task 9.2.3).
//!
//! Everything here is GPUI-agnostic pure logic over the vendored encoders,
//! so every behaviour below is a plain `#[test]`:
//!
//! - [`ImeState`] buffers composition text and emits it exactly once, whole,
//!   on commit — partial sequences never reach the PTY.
//! - [`MouseEncoder`] wraps the vendored mouse encoder: tracking mode and
//!   format sync from the terminal, geometry comes from the surface.
//! - [`wrap_paste`] routes paste bytes through bracketed paste when the
//!   terminal enabled mode 2004, else the plain path (newlines become CR).
//! - [`ClipboardBackend`] is the injectable clipboard interface from the
//!   component contract (`docs/terminal-component.md`): the app performs the
//!   platform operation, the component only detects OSC 52 (see
//!   [`Terminal::on_clipboard_write`](crate::terminal::Terminal::on_clipboard_write)).
//!   [`MemoryClipboard`] is the test double.
//!
//! GPUI event wiring (`KeyDownEvent` → [`KeyPress`](super::keys::KeyPress),
//! mouse events → cell coordinates) lands with the 9.3 embedding, not here.

pub use libghostty_vt::mouse::{
    Action as MouseAction, Button as MouseButton, Format as MouseFormat,
    TrackingMode as MouseTrackingMode,
};
pub use libghostty_vt::paste::is_safe as paste_is_safe;

use libghostty_vt::mouse as vt_mouse;
use libghostty_vt::paste as vt_paste;

use crate::terminal::{Error, Mode, Terminal, VtTerminal};

/// IME composition buffer.
///
/// The app feeds platform composition updates here and forwards only what
/// [`ImeState::commit`] returns. Nothing is emitted mid-composition, so a
/// CJK candidate window can never inject a half-formed sequence.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImeState {
    composing: bool,
    buffer: String,
}

impl ImeState {
    /// Idle state, no composition in flight.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a composition is currently open.
    #[must_use]
    pub const fn is_composing(&self) -> bool {
        self.composing
    }

    /// Start (or restart) a composition, dropping any previous buffer.
    pub fn begin(&mut self) {
        self.composing = true;
        self.buffer.clear();
    }

    /// Replace the in-progress buffer. Emits nothing.
    pub fn push_composition(&mut self, text: &str) {
        self.composing = true;
        self.buffer.clear();
        self.buffer.push_str(text);
    }

    /// Finish the composition, returning the whole text exactly once.
    ///
    /// Returns `None` when nothing was composed. The state goes idle, so a
    /// second `commit` without new input yields `None`, never a repeat.
    pub fn commit(&mut self) -> Option<String> {
        self.composing = false;
        if self.buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buffer))
        }
    }

    /// Abandon the composition, emitting nothing.
    pub fn cancel(&mut self) {
        self.composing = false;
        self.buffer.clear();
    }
}

/// Rendered geometry the mouse encoder needs to turn pixels into cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseGeometry {
    /// Viewport width in pixels.
    pub screen_width_px: u32,
    /// Viewport height in pixels.
    pub screen_height_px: u32,
    /// One cell width in pixels (non-zero).
    pub cell_width_px: u32,
    /// One cell height in pixels (non-zero).
    pub cell_height_px: u32,
}

impl MouseGeometry {
    /// Build from surface metrics: viewport box plus measured cell box.
    #[must_use]
    pub const fn new(
        screen_width_px: u32,
        screen_height_px: u32,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Self {
        Self {
            screen_width_px,
            screen_height_px,
            cell_width_px,
            cell_height_px,
        }
    }
}

/// One mouse event in cell coordinates (0-indexed, viewport-relative).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellMouseEvent {
    /// Press, release, or motion.
    pub action: MouseAction,
    /// Button involved (`None` for motion with no button).
    pub button: Option<MouseButton>,
    /// Held keyboard modifiers, from [`Mods`](super::keys::Mods).
    pub mods: libghostty_vt::key::Mods,
    /// Cell column.
    pub col: u16,
    /// Cell row.
    pub row: u16,
}

/// Mouse encoder synced to terminal tracking modes.
pub struct MouseEncoder<'alloc> {
    inner: vt_mouse::Encoder<'alloc>,
    geometry: Option<MouseGeometry>,
}

impl MouseEncoder<'_> {
    /// Create an encoder with no geometry yet (encode fails until set).
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            inner: vt_mouse::Encoder::new()?,
            geometry: None,
        })
    }

    /// Set the render geometry (viewport + cell box).
    pub fn set_geometry(&mut self, geometry: MouseGeometry) -> &mut Self {
        self.inner.set_size(vt_mouse::EncoderSize {
            screen_width: geometry.screen_width_px,
            screen_height: geometry.screen_height_px,
            cell_width: geometry.cell_width_px,
            cell_height: geometry.cell_height_px,
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        });
        self.geometry = Some(geometry);
        self
    }

    /// Sync tracking mode and format from the terminal's current state.
    pub fn sync_from(&mut self, terminal: &Terminal) -> &mut Self {
        self.inner.set_options_from_terminal(terminal.inner());
        self
    }

    /// Sync from a bare [`VtTerminal`] (tests and non-`Terminal` owners).
    pub fn sync_from_vt(&mut self, terminal: &VtTerminal<'static, 'static>) -> &mut Self {
        self.inner.set_options_from_terminal(terminal);
        self
    }

    /// Override the output format (X10, SGR, ...).
    ///
    /// [`sync_from`](Self::sync_from) leaves the format at the encoder
    /// default (X10); call this after syncing when the program selected a
    /// format the terminal state does not carry.
    pub fn set_format(&mut self, format: MouseFormat) -> &mut Self {
        self.inner.set_format(format);
        self
    }

    /// Encode one cell event into the bytes to write to the PTY.
    ///
    /// Returns an empty vec when tracking is off or the event produces no
    /// output (e.g. motion in press-only mode). Requires geometry first.
    pub fn encode_event(&mut self, event: &CellMouseEvent) -> Result<Vec<u8>, Error> {
        let Some(geometry) = self.geometry else {
            return Err(Error::InvalidValue);
        };
        let mut vt_event = vt_mouse::Event::new()?;
        vt_event
            .set_action(event.action)
            .set_button(event.button)
            .set_mods(event.mods)
            .set_position(vt_mouse::Position {
                x: f32::from(event.col) * geometry.cell_width_px as f32,
                y: f32::from(event.row) * geometry.cell_height_px as f32,
            });
        let mut out = Vec::new();
        self.inner.encode_to_vec(&vt_event, &mut out)?;
        Ok(out)
    }
}

/// Whether the terminal enabled bracketed paste (mode 2004).
pub fn bracketed_paste_enabled(terminal: &VtTerminal<'static, 'static>) -> Result<bool, Error> {
    terminal.mode(Mode::BRACKETED_PASTE)
}

/// Encode paste bytes for the PTY.
///
/// `bracketed` should come from [`bracketed_paste_enabled`]. The bracketed
/// path wraps in `ESC[200~` … `ESC[201~` and strips unsafe control bytes;
/// the plain path additionally folds newlines to CR. Grows the output
/// buffer on demand.
pub fn wrap_paste(data: &str, bracketed: bool) -> Result<Vec<u8>, Error> {
    let mut src = data.as_bytes().to_vec();
    // Bracket markers add 12 bytes; plain path never grows the payload.
    let mut buf = vec![0u8; src.len().max(1) + 12];
    loop {
        match vt_paste::encode(&mut src, bracketed, &mut buf) {
            Ok(n) => {
                buf.truncate(n);
                return Ok(buf);
            }
            Err(Error::OutOfSpace { required }) => {
                buf.resize(required, 0);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Clipboard access goes through the host app.
///
/// The component detects OSC 52 writes and reports size queries, but never
/// touches a platform clipboard itself — the app injects its backend here.
pub trait ClipboardBackend {
    /// Current clipboard text, if any.
    fn read_text(&self) -> Option<String>;
    /// Replace the clipboard text.
    fn write_text(&mut self, text: &str);
}

/// In-memory clipboard for tests and headless hosts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryClipboard {
    content: Option<String>,
}

impl MemoryClipboard {
    /// Empty clipboard.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ClipboardBackend for MemoryClipboard {
    fn read_text(&self) -> Option<String> {
        self.content.clone()
    }

    fn write_text(&mut self, text: &str) {
        self.content = Some(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::TerminalConfig;
    use libghostty_vt::key::Mods;

    fn test_terminal() -> Result<Terminal, Error> {
        Terminal::new(TerminalConfig {
            cols: 80,
            rows: 24,
            max_scrollback: 100,
            ..TerminalConfig::default()
        })
    }

    fn geometry() -> MouseGeometry {
        MouseGeometry::new(800, 600, 8, 16)
    }

    #[test]
    fn ime_emits_whole_text_once_on_commit() -> Result<(), Error> {
        let mut ime = ImeState::new();
        assert!(!ime.is_composing());
        ime.begin();
        assert!(ime.is_composing());
        // Partial updates accumulate silently: nothing to forward yet.
        ime.push_composition("ni");
        ime.push_composition("nihon");
        assert!(ime.is_composing());
        assert_eq!(ime.commit().as_deref(), Some("nihon"));
        assert!(!ime.is_composing());
        // No repeat without new input.
        assert_eq!(ime.commit(), None);
        Ok(())
    }

    #[test]
    fn ime_cancel_emits_nothing() {
        let mut ime = ImeState::new();
        ime.begin();
        ime.push_composition("han");
        ime.cancel();
        assert!(!ime.is_composing());
        assert_eq!(ime.commit(), None);
    }

    #[test]
    fn mouse_stays_silent_without_tracking() -> Result<(), Error> {
        let term = test_terminal()?;
        let mut enc = MouseEncoder::new()?;
        enc.set_geometry(geometry()).sync_from(&term);
        let press = CellMouseEvent {
            action: MouseAction::Press,
            button: Some(MouseButton::Left),
            mods: Mods::empty(),
            col: 10,
            row: 5,
        };
        assert!(enc.encode_event(&press)?.is_empty());
        Ok(())
    }

    #[test]
    fn mouse_reports_x10_by_default_sgr_on_request() -> Result<(), Error> {
        let mut term = test_terminal()?;
        term.feed(b"\x1b[?1000h");
        let mut enc = MouseEncoder::new()?;
        enc.set_geometry(geometry()).sync_from(&term);
        let press = CellMouseEvent {
            action: MouseAction::Press,
            button: Some(MouseButton::Left),
            mods: Mods::empty(),
            col: 10,
            row: 5,
        };
        // Default format is X10: ESC M Cb Cx Cy, coords 1-based.
        assert_eq!(enc.encode_event(&press)?, b"\x1b[M +&");
        // Release also reports in normal mode.
        let release = CellMouseEvent {
            action: MouseAction::Release,
            button: Some(MouseButton::Left),
            ..press
        };
        let rel = enc.encode_event(&release)?;
        assert!(rel.starts_with(b"\x1b[M"), "{rel:?}");
        assert_eq!(rel.len(), 6);
        // Plain motion does not (press-only mode).
        let motion = CellMouseEvent {
            action: MouseAction::Motion,
            button: None,
            ..press
        };
        assert!(enc.encode_event(&motion)?.is_empty());
        // Explicit SGR format: ESC[< Cb ; Cx ; Cy M, coords 1-based.
        enc.set_format(MouseFormat::Sgr);
        assert_eq!(enc.encode_event(&press)?, b"\x1b[<0;11;6M");
        Ok(())
    }

    #[test]
    fn mouse_needs_geometry_first() -> Result<(), Error> {
        let term = test_terminal()?;
        let mut enc = MouseEncoder::new()?;
        enc.sync_from(&term);
        let press = CellMouseEvent {
            action: MouseAction::Press,
            button: Some(MouseButton::Left),
            mods: Mods::empty(),
            col: 0,
            row: 0,
        };
        assert!(enc.encode_event(&press).is_err());
        Ok(())
    }

    #[test]
    fn bracketed_paste_wraps_and_plain_folds() -> Result<(), Error> {
        let mut term = test_terminal()?;
        assert!(!bracketed_paste_enabled(term.inner())?);
        let plain = wrap_paste("a\nb", false)?;
        assert_eq!(plain, b"a\rb");
        term.feed(b"\x1b[?2004h");
        assert!(bracketed_paste_enabled(term.inner())?);
        let wrapped = wrap_paste("a\nb", true)?;
        assert!(wrapped.starts_with(b"\x1b[200~"), "{wrapped:?}");
        assert!(wrapped.ends_with(b"\x1b[201~"), "{wrapped:?}");
        assert!(wrapped.windows(3).any(|w| w == b"a\nb"), "{wrapped:?}");
        term.feed(b"\x1b[?2004l");
        assert!(!bracketed_paste_enabled(term.inner())?);
        Ok(())
    }

    #[test]
    fn paste_safety_flags_injection() {
        assert!(paste_is_safe("hello world"));
        assert!(!paste_is_safe("rm -rf /\n"));
        assert!(!paste_is_safe("ok\x1b[201~rm"));
    }

    #[test]
    fn clipboard_roundtrips_through_backend() {
        let mut clip = MemoryClipboard::new();
        assert_eq!(clip.read_text(), None);
        clip.write_text("copied");
        assert_eq!(clip.read_text().as_deref(), Some("copied"));
        // OSC 52 payloads flow app-side through the same interface.
        clip.write_text("from-osc52");
        assert_eq!(clip.read_text().as_deref(), Some("from-osc52"));
    }
}
