//! Mode-aware key encoding (task 9.2.3).
//!
//! [`KeyEncoder`] wraps the vendored [`key::Encoder`]. The app syncs it from
//! the live [`Terminal`](crate::terminal::Terminal) before encoding (or after
//! any mode change), so arrows, keypad, and Kitty-protocol keys follow the
//! modes the running program actually set. [`KeyPress`] is the GPUI-agnostic
//! input event; [`key_press_from_keystroke`] translates a GPUI
//! [`Keystroke`](gpui::Keystroke) into it (task 9.3.5), which keeps every
//! test here a plain `#[test]`.

pub use libghostty_vt::key::{Action as KeyAction, Key, KittyKeyFlags, Mods, OptionAsAlt};

use libghostty_vt::key as vt_key;

use crate::terminal::{Error, Terminal, VtTerminal};

/// One key press to encode.
///
/// `text` is the unmodified character the layout produced (`None` for
/// non-printable keys). The encoder derives modifier sequences from
/// [`KeyPress::key`] plus [`KeyPress::mods`], never from the text, so pass
/// the pre-Ctrl character (`"c"`, not `"\x03"`) and never a C0 control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPress {
    /// Physical/logical key (arrows, function keys, letters, ...).
    pub key: Key,
    /// Layout-produced text, if any.
    pub text: Option<String>,
    /// Held modifiers.
    pub mods: Mods,
    /// Part of an IME composition sequence (encoder may suppress output).
    pub composing: bool,
}

impl KeyPress {
    /// A non-printable key with no modifiers.
    #[must_use]
    pub fn simple(key: Key) -> Self {
        Self {
            key,
            text: None,
            mods: Mods::empty(),
            composing: false,
        }
    }

    /// Attach layout-produced text (printable keys).
    #[must_use]
    pub fn with_text(mut self, text: &str) -> Self {
        self.text = Some(text.to_string());
        self
    }

    /// Attach held modifiers.
    #[must_use]
    pub fn with_mods(mut self, mods: Mods) -> Self {
        self.mods = mods;
        self
    }
}

/// Translate a GPUI keystroke into a [`KeyPress`].
///
/// GPUI names keys by what the layout prints (`"a"`, `"enter"`, `"up"`,
/// `"f5"`) and reports the typed character separately in `key_char`, with
/// C0 controls already filtered out (Ctrl+C arrives as key `"c"`,
/// `key_char: None`). The encoder wants the logical [`Key`] plus the
/// pre-Ctrl text, so a single-character key name doubles as the text when
/// `key_char` is absent. Keys with no physical-key equivalent (shifted
/// punctuation GPUI already resolved, non-ASCII layouts) map to
/// [`Key::Unidentified`] and ride on their text alone.
#[must_use]
pub fn key_press_from_keystroke(keystroke: &gpui::Keystroke) -> KeyPress {
    let name = keystroke.key.as_str();
    let key = key_from_gpui_name(name);
    let text = keystroke
        .key_char
        .clone()
        .or_else(|| (name.chars().count() == 1).then(|| name.to_string()));
    let m = keystroke.modifiers;
    let mut mods = Mods::empty();
    mods.set(Mods::SHIFT, m.shift);
    mods.set(Mods::CTRL, m.control);
    mods.set(Mods::ALT, m.alt);
    mods.set(Mods::SUPER, m.platform);
    KeyPress {
        key,
        text,
        mods,
        composing: false,
    }
}

/// Map a GPUI key name to the VT logical key.
fn key_from_gpui_name(name: &str) -> Key {
    match name {
        "enter" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "escape" => Key::Escape,
        "space" => Key::Space,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        "shift" => Key::ShiftLeft,
        "control" => Key::ControlLeft,
        "alt" => Key::AltLeft,
        "platform" => Key::MetaLeft,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        _ => {
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => key_from_char(c),
                _ => Key::Unidentified,
            }
        }
    }
}

/// Map a single unshifted US-layout character to its physical key.
fn key_from_char(c: char) -> Key {
    const LETTERS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    const DIGITS: [Key; 10] = [
        Key::Digit0,
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
        Key::Digit6,
        Key::Digit7,
        Key::Digit8,
        Key::Digit9,
    ];
    let lower = c.to_ascii_lowercase();
    if lower.is_ascii_lowercase() {
        return LETTERS[(lower as u8 - b'a') as usize];
    }
    if c.is_ascii_digit() {
        return DIGITS[(c as u8 - b'0') as usize];
    }
    match c {
        '`' => Key::Backquote,
        '\\' => Key::Backslash,
        '[' => Key::BracketLeft,
        ']' => Key::BracketRight,
        ',' => Key::Comma,
        '=' => Key::Equal,
        '-' => Key::Minus,
        '.' => Key::Period,
        '\'' => Key::Quote,
        ';' => Key::Semicolon,
        '/' => Key::Slash,
        ' ' => Key::Space,
        _ => Key::Unidentified,
    }
}

/// Key encoder synced to terminal modes.
///
/// Owns a [`vt_key::Encoder`]. Call [`KeyEncoder::sync_from`] after creation
/// and after any mode change the app knows about (the encoder reads cursor/
/// keypad/alt/modifyOtherKeys/Kitty state straight from the terminal).
pub struct KeyEncoder<'alloc> {
    inner: vt_key::Encoder<'alloc>,
}

impl KeyEncoder<'_> {
    /// Create an encoder with default options (normal cursor keys, no Kitty).
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            inner: vt_key::Encoder::new()?,
        })
    }

    /// Sync encoder options from the terminal's current modes.
    ///
    /// Resets `macos_option_as_alt` to off (it is not terminal state); set
    /// it explicitly after this call when the app wants it.
    pub fn sync_from(&mut self, terminal: &Terminal) -> &mut Self {
        self.inner.set_options_from_terminal(terminal.inner());
        self
    }

    /// Sync from a bare [`VtTerminal`] (tests and non-`Terminal` owners).
    pub fn sync_from_vt(&mut self, terminal: &VtTerminal<'static, 'static>) -> &mut Self {
        self.inner.set_options_from_terminal(terminal);
        self
    }

    /// Encode one press into the bytes to write to the PTY.
    ///
    /// Returns an empty vec when the key produces no output (bare modifiers).
    pub fn encode_press(&mut self, press: &KeyPress) -> Result<Vec<u8>, Error> {
        let mut event = vt_key::Event::new()?;
        event
            .set_action(KeyAction::Press)
            .set_key(press.key)
            .set_mods(press.mods)
            .set_composing(press.composing);
        match &press.text {
            Some(text) => {
                if let Some(first) = text.chars().next() {
                    event.set_unshifted_codepoint(first);
                }
                event.set_utf8(Some(text.clone()));
            }
            None => {
                event.set_utf8(Option::<&str>::None);
            }
        }
        let mut out = Vec::new();
        self.inner.encode_to_vec(&event, &mut out)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::TerminalConfig;

    fn test_terminal() -> Result<Terminal, Error> {
        Terminal::new(TerminalConfig {
            cols: 80,
            rows: 24,
            max_scrollback: 100,
            ..TerminalConfig::default()
        })
    }

    fn synced() -> Result<(Terminal, KeyEncoder<'static>), Error> {
        let term = test_terminal()?;
        let mut enc = KeyEncoder::new()?;
        enc.sync_from(&term);
        Ok((term, enc))
    }

    #[test]
    fn arrows_encode_normal_mode() -> Result<(), Error> {
        let (_term, mut enc) = synced()?;
        let cases = [
            (Key::ArrowUp, b"\x1b[A".as_slice()),
            (Key::ArrowDown, b"\x1b[B".as_slice()),
            (Key::ArrowRight, b"\x1b[C".as_slice()),
            (Key::ArrowLeft, b"\x1b[D".as_slice()),
        ];
        for (key, expected) in cases {
            assert_eq!(enc.encode_press(&KeyPress::simple(key))?, expected);
        }
        Ok(())
    }

    #[test]
    fn arrows_follow_application_cursor_mode() -> Result<(), Error> {
        let mut term = test_terminal()?;
        term.feed(b"\x1b[?1h");
        let mut enc = KeyEncoder::new()?;
        enc.sync_from(&term);
        let cases = [
            (Key::ArrowUp, b"\x1bOA".as_slice()),
            (Key::ArrowDown, b"\x1bOB".as_slice()),
            (Key::ArrowRight, b"\x1bOC".as_slice()),
            (Key::ArrowLeft, b"\x1bOD".as_slice()),
        ];
        for (key, expected) in cases {
            assert_eq!(enc.encode_press(&KeyPress::simple(key))?, expected);
        }
        // Back to normal mode tracks too.
        term.feed(b"\x1b[?1l");
        enc.sync_from(&term);
        assert_eq!(
            enc.encode_press(&KeyPress::simple(Key::ArrowUp))?,
            b"\x1b[A"
        );
        Ok(())
    }

    #[test]
    fn editing_keys_encode_xterm_sequences() -> Result<(), Error> {
        let (_term, mut enc) = synced()?;
        let cases = [
            (Key::Home, b"\x1b[H".as_slice()),
            (Key::End, b"\x1b[F".as_slice()),
            (Key::Insert, b"\x1b[2~".as_slice()),
            (Key::Delete, b"\x1b[3~".as_slice()),
            (Key::PageUp, b"\x1b[5~".as_slice()),
            (Key::PageDown, b"\x1b[6~".as_slice()),
            (Key::F1, b"\x1bOP".as_slice()),
            (Key::F5, b"\x1b[15~".as_slice()),
        ];
        for (key, expected) in cases {
            assert_eq!(enc.encode_press(&KeyPress::simple(key))?, expected);
        }
        Ok(())
    }

    #[test]
    fn control_keys_encode_c0_bytes() -> Result<(), Error> {
        let (_term, mut enc) = synced()?;
        assert_eq!(enc.encode_press(&KeyPress::simple(Key::Enter))?, b"\r");
        assert_eq!(enc.encode_press(&KeyPress::simple(Key::Tab))?, b"\t");
        assert_eq!(
            enc.encode_press(&KeyPress::simple(Key::Backspace))?,
            b"\x7f"
        );
        assert_eq!(enc.encode_press(&KeyPress::simple(Key::Escape))?, b"\x1b");
        Ok(())
    }

    #[test]
    fn printable_and_ctrl_keys() -> Result<(), Error> {
        let (_term, mut enc) = synced()?;
        assert_eq!(
            enc.encode_press(&KeyPress::simple(Key::A).with_text("a"))?,
            b"a"
        );
        assert_eq!(
            enc.encode_press(
                &KeyPress::simple(Key::A)
                    .with_text("A")
                    .with_mods(Mods::SHIFT)
            )?,
            b"A"
        );
        // Ctrl+C: logical key + mods, pre-Ctrl text; encoder yields 0x03.
        assert_eq!(
            enc.encode_press(
                &KeyPress::simple(Key::C)
                    .with_text("c")
                    .with_mods(Mods::CTRL)
            )?,
            b"\x03"
        );
        Ok(())
    }

    fn keystroke(key: &str, key_char: Option<&str>, modifiers: gpui::Modifiers) -> gpui::Keystroke {
        gpui::Keystroke {
            modifiers,
            key: key.to_string(),
            key_char: key_char.map(ToString::to_string),
        }
    }

    #[test]
    fn gpui_keystrokes_translate_and_encode() -> Result<(), Error> {
        let (_term, mut enc) = synced()?;
        let none = gpui::Modifiers::default();
        let ctrl = gpui::Modifiers {
            control: true,
            ..gpui::Modifiers::default()
        };
        let shift = gpui::Modifiers {
            shift: true,
            ..gpui::Modifiers::default()
        };
        let cases: [(gpui::Keystroke, &[u8]); 8] = [
            (keystroke("a", Some("a"), none), b"a"),
            (keystroke("a", Some("A"), shift), b"A"),
            // GPUI already resolved Shift+1 to "!" and dropped the shift.
            (keystroke("!", Some("!"), none), b"!"),
            (keystroke("space", Some(" "), none), b" "),
            // Ctrl+C: GPUI filters the C0 char, so the key name is the text.
            (keystroke("c", None, ctrl), b"\x03"),
            (keystroke("enter", None, none), b"\r"),
            (keystroke("backspace", None, none), b"\x7f"),
            (keystroke("up", None, none), b"\x1b[A"),
        ];
        for (stroke, expected) in cases {
            let press = key_press_from_keystroke(&stroke);
            assert_eq!(enc.encode_press(&press)?, expected, "{stroke:?}");
        }
        Ok(())
    }

    #[test]
    fn gpui_key_names_map_to_logical_keys() {
        assert_eq!(key_from_gpui_name("f5"), Key::F5);
        assert_eq!(key_from_gpui_name("pagedown"), Key::PageDown);
        assert_eq!(key_from_gpui_name("Z"), Key::Z);
        assert_eq!(key_from_gpui_name("7"), Key::Digit7);
        assert_eq!(key_from_gpui_name("/"), Key::Slash);
        assert_eq!(key_from_gpui_name("é"), Key::Unidentified);
        assert_eq!(key_from_gpui_name("volumeup"), Key::Unidentified);
    }

    #[test]
    fn bare_modifiers_emit_nothing() -> Result<(), Error> {
        let (_term, mut enc) = synced()?;
        assert!(
            enc.encode_press(&KeyPress::simple(Key::ShiftLeft))?
                .is_empty()
        );
        assert!(
            enc.encode_press(&KeyPress::simple(Key::ControlLeft))?
                .is_empty()
        );
        Ok(())
    }
}
