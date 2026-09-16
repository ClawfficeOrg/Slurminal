//! `CodeEditor` component for generated GPUI starters.
//!
//! Provides a syntax-highlighted, editable code pane for Rust and TOML sources.
//!
//! A lightweight pure-Rust state-machine tokenizer is used instead of
//! tree-sitter to avoid external C compilation dependencies on all target
//! platforms (including the Windows MSVC CI runner).  The decision is
//! documented in `docs/memory.md`.
//!
//! # Features
//!
//! - Syntax highlighting for Rust and TOML via a built-in line-by-line lexer.
//! - Optional line-number gutter.
//! - Current-line background highlight.
//! - Bracket matching: `()`, `[]`, `{}` pairs are highlighted when the cursor
//!   is adjacent to a bracket character (strings/comments are excluded).
//! - Editable mode: keyboard character input, backspace, enter, and arrow-key
//!   navigation.  All editing is Unicode-scalar-correct (cursor tracks
//!   char indices, not byte offsets).
//! - Read-only mode: disables all edit operations.
//! - Autocomplete stub: pressing Ctrl+Space invokes the registered
//!   `on_complete` callback and shows a minimal suggestion overlay.
//!
//! # Scope
//!
//! Rust and TOML tokenizers only.  Multi-language support and full tree-sitter
//! integration are deferred to a future task.
//!
//! # Headless tests
//!
//! Use the `apply_*` mutator methods to drive state changes from
//! [`gpui::TestAppContext`] without a `&mut Window`.
//!
//! # GPUI version
//!
//! Implemented against `gpui 0.2.2`.

use std::rc::Rc;

use gpui::{
    App, Context, ElementId, FocusHandle, Focusable, IntoElement, KeyDownEvent, Render,
    SharedString, Window, div, prelude::*, px,
};

use crate::ui_tokens::{FocusRingMetrics, RadiusScale};
use gpui_kit::component::{Theme, ThemeColor};

// ─────────────────────────────────────────────────────────────────────────────
// Token kinds
// ─────────────────────────────────────────────────────────────────────────────

/// Syntax category controlling the rendered color of a [`TokenSpan`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    /// Reserved language keyword (`fn`, `let`, `if`, `[package]`, etc.).
    Keyword,
    /// Built-in type name (`String`, `Vec`, `Option`, `i32`, etc.).
    TypeName,
    /// Line comment (`//…` for Rust, `#…` for TOML).
    Comment,
    /// String literal (`"…"` or `'…'`).
    Str,
    /// Numeric literal.
    Number,
    /// Operator character (`+`, `-`, `*`, `/`, `=`, `!`, `&`, `|`, `<`, `>`, `^`, `?`).
    Operator,
    /// Punctuation (`;`, `,`, `.`, `:`).
    Punctuation,
    /// TOML section header (`[section]` or `[[array]]`).
    Section,
    /// TOML key — the left-hand side of a `key = value` pair.
    Key,
    /// Boolean literal (`true` / `false`).
    Boolean,
    /// Bracket character — `(`, `)`, `[`, `]`, `{`, `}`.
    ///
    /// Each bracket is its own single-character span so the bracket-match
    /// renderer can apply a highlight background without span splitting.
    Bracket,
    /// Unclassified identifier, whitespace, or other text.
    Plain,
}

/// A single classified span of text within one source line.
///
/// Spans are contiguous and non-overlapping; concatenating all spans for a
/// line reconstructs the original source line verbatim.
#[derive(Clone, Debug, PartialEq)]
pub struct TokenSpan {
    /// Display category controlling the rendered color.
    pub kind: TokenKind,
    /// The original source text of this span.
    pub text: String,
}

impl TokenSpan {
    fn new(kind: TokenKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bracket pair
// ─────────────────────────────────────────────────────────────────────────────

/// A matched open/close bracket pair, expressed as `(line, char_col)` pairs.
///
/// Both coordinates are zero-indexed.  The char column is a Unicode scalar
/// offset within the line string, matching the cursor coordinate system.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BracketPair {
    /// Zero-indexed line of the opening bracket.
    pub open_line: usize,
    /// Zero-indexed char column of the opening bracket.
    pub open_col: usize,
    /// Zero-indexed line of the closing bracket.
    pub close_line: usize,
    /// Zero-indexed char column of the closing bracket.
    pub close_col: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Language
// ─────────────────────────────────────────────────────────────────────────────

/// Tokenizer language selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorLanguage {
    /// Rust source file (`.rs`).
    Rust,
    /// TOML configuration file (`.toml`).
    Toml,
}

impl EditorLanguage {
    /// Human-readable label for the language.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Toml => "TOML",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Color tokens
// ─────────────────────────────────────────────────────────────────────────────

/// Token-resolved display colors for a [`CodeEditor`].
///
/// Syntax-token colors are taken from a VS Code–inspired dark/light palette
/// baked in at construction time from the `dark` flag.
#[derive(Clone, Copy, Debug)]
pub struct CodeEditorColors {
    /// Editor surface background.
    pub background: gpui::Hsla,
    /// Border around the editor widget.
    pub border: gpui::Hsla,
    /// Primary text color (plain tokens, operators).
    pub text: gpui::Hsla,
    /// Gutter strip background.
    pub gutter_bg: gpui::Hsla,
    /// Gutter line-number text color.
    pub gutter_text: gpui::Hsla,
    /// Background highlight on the line at cursor position.
    pub current_line_bg: gpui::Hsla,
    /// Background on a matched bracket character.
    pub bracket_match_bg: gpui::Hsla,
    /// Focus ring color.
    pub focus_ring: gpui::Hsla,
    /// Header bar background (language + mode label).
    pub header_bg: gpui::Hsla,
    /// Header bar text color.
    pub header_text: gpui::Hsla,
    // ── Syntax token colors ────────────────────────────────────────────────
    /// Color for keywords (`fn`, `let`, `for`, `pub`, etc.).
    pub syn_keyword: gpui::Hsla,
    /// Color for built-in type names (`String`, `Vec`, `i32`, etc.).
    pub syn_type: gpui::Hsla,
    /// Color for line comments.
    pub syn_comment: gpui::Hsla,
    /// Color for string literals.
    pub syn_string: gpui::Hsla,
    /// Color for numeric literals.
    pub syn_number: gpui::Hsla,
    /// Color for operators.
    pub syn_operator: gpui::Hsla,
    /// Color for bracket characters when not part of a matched pair.
    pub syn_bracket: gpui::Hsla,
    /// Color for TOML section headers.
    pub syn_section: gpui::Hsla,
    /// Color for TOML keys.
    pub syn_key: gpui::Hsla,
    /// Color for boolean literals.
    pub syn_boolean: gpui::Hsla,
    // ── Autocomplete overlay ───────────────────────────────────────────────
    /// Background of the autocomplete suggestion panel.
    pub completion_bg: gpui::Hsla,
    /// Border of the autocomplete suggestion panel.
    pub completion_border: gpui::Hsla,
    /// Text color inside the completion panel.
    pub completion_text: gpui::Hsla,
}

impl CodeEditorColors {
    /// Resolves editor colors from the given `tokens` and `dark` flag.
    ///
    /// Syntax token colors use a VS Code–inspired palette independent of the
    /// application theme to match widely understood coding conventions.
    #[must_use]
    pub fn resolve(theme: &Theme, dark: bool) -> Self {
        // Helper: hex color to Hsla.  The hex value is 0xRRGGBB.
        fn hex(rgb: u32) -> gpui::Hsla {
            let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
            let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
            let b = (rgb & 0xff) as f32 / 255.0;
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let l = (max + min) / 2.0;
            let s = if (max - min).abs() < f32::EPSILON {
                0.0
            } else if l < 0.5 {
                (max - min) / (max + min)
            } else {
                (max - min) / (2.0 - max - min)
            };
            let h = if (max - min).abs() < f32::EPSILON {
                0.0
            } else if (max - r).abs() < f32::EPSILON {
                ((g - b) / (max - min)).rem_euclid(6.0) / 6.0
            } else if (max - g).abs() < f32::EPSILON {
                ((b - r) / (max - min) + 2.0) / 6.0
            } else {
                ((r - g) / (max - min) + 4.0) / 6.0
            };
            gpui::Hsla { h, s, l, a: 1.0 }
        }

        if dark {
            Self {
                background: hex(0x1e1e1e),
                border: theme.border,
                text: hex(0xd4d4d4),
                gutter_bg: hex(0x1e1e1e),
                gutter_text: hex(0x858585),
                current_line_bg: hex(0x2a2a2a),
                bracket_match_bg: hex(0x3b5998),
                focus_ring: FocusRingMetrics::dark().color,
                header_bg: hex(0x252526),
                header_text: hex(0xcccccc),
                syn_keyword: hex(0x569cd6),
                syn_type: hex(0x4ec9b0),
                syn_comment: hex(0x6a9955),
                syn_string: hex(0xce9178),
                syn_number: hex(0xb5cea8),
                syn_operator: hex(0xd4d4d4),
                syn_bracket: hex(0xffd700),
                syn_section: hex(0x569cd6),
                syn_key: hex(0x9cdcfe),
                syn_boolean: hex(0x569cd6),
                completion_bg: hex(0x252526),
                completion_border: hex(0x454545),
                completion_text: hex(0xd4d4d4),
            }
        } else {
            Self {
                background: hex(0xffffff),
                border: theme.border,
                text: hex(0x000000),
                gutter_bg: hex(0xf0f0f0),
                gutter_text: hex(0x999999),
                current_line_bg: hex(0xeaeaff),
                bracket_match_bg: hex(0xc0dafe),
                focus_ring: FocusRingMetrics::light().color,
                header_bg: hex(0xf3f3f3),
                header_text: hex(0x333333),
                syn_keyword: hex(0x0000ff),
                syn_type: hex(0x267f99),
                syn_comment: hex(0x008000),
                syn_string: hex(0xa31515),
                syn_number: hex(0x098658),
                syn_operator: hex(0x000000),
                syn_bracket: hex(0x7c4dff),
                syn_section: hex(0x0000ff),
                syn_key: hex(0x001080),
                syn_boolean: hex(0x0000ff),
                completion_bg: hex(0xfafafa),
                completion_border: hex(0xc8c8c8),
                completion_text: hex(0x000000),
            }
        }
    }

    /// Returns the color for the given [`TokenKind`].
    #[must_use]
    pub fn token_color(&self, kind: TokenKind) -> gpui::Hsla {
        match kind {
            TokenKind::Keyword => self.syn_keyword,
            TokenKind::TypeName => self.syn_type,
            TokenKind::Comment => self.syn_comment,
            TokenKind::Str => self.syn_string,
            TokenKind::Number => self.syn_number,
            TokenKind::Operator => self.syn_operator,
            TokenKind::Punctuation => self.text,
            TokenKind::Section => self.syn_section,
            TokenKind::Key => self.syn_key,
            TokenKind::Boolean => self.syn_boolean,
            TokenKind::Bracket => self.syn_bracket,
            TokenKind::Plain => self.text,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Callbacks
// ─────────────────────────────────────────────────────────────────────────────

/// Callback fired when the source text changes (edit operations).
///
/// Receives the full current source text and mutable window + app contexts.
pub type CodeChangeHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// Callback stub fired when the user triggers autocomplete (Ctrl+Space).
///
/// Receives the current source text.  No suggestions are provided by the
/// component itself — the callback is an integration point for the host
/// application.
pub type CompletionHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

// ─────────────────────────────────────────────────────────────────────────────
// Tokenizer: Rust
// ─────────────────────────────────────────────────────────────────────────────

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "type", "union", "unsafe",
    "use", "where", "while",
];

const RUST_TYPES: &[&str] = &[
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8", "u16",
    "u32", "u64", "u128", "usize", "Arc", "Box", "Cell", "HashMap", "HashSet", "Option", "Rc",
    "RefCell", "Result", "String", "Vec",
];

/// Tokenizes one line of Rust source.
///
/// Returns a flat list of [`TokenSpan`]s whose texts concatenate to `line`.
#[must_use]
pub fn tokenize_rust_line(line: &str) -> Vec<TokenSpan> {
    let mut spans: Vec<TokenSpan> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        // Line comment
        if i + 1 < n && chars[i] == '/' && chars[i + 1] == '/' {
            let rest: String = chars[i..].iter().collect();
            spans.push(TokenSpan::new(TokenKind::Comment, rest));
            break;
        }

        // String literal
        if chars[i] == '"' {
            let mut s = String::new();
            s.push('"');
            i += 1;
            while i < n {
                let c = chars[i];
                s.push(c);
                if c == '\\' && i + 1 < n {
                    i += 1;
                    s.push(chars[i]);
                } else if c == '"' {
                    break;
                }
                i += 1;
            }
            spans.push(TokenSpan::new(TokenKind::Str, s));
            i += 1;
            continue;
        }

        // Bracket characters (each emitted as its own span)
        if matches!(chars[i], '(' | ')' | '[' | ']' | '{' | '}') {
            spans.push(TokenSpan::new(TokenKind::Bracket, chars[i].to_string()));
            i += 1;
            continue;
        }

        // Operator characters
        if matches!(
            chars[i],
            '+' | '-' | '*' | '/' | '=' | '!' | '&' | '|' | '<' | '>' | '^' | '~' | '@' | '?'
        ) {
            spans.push(TokenSpan::new(TokenKind::Operator, chars[i].to_string()));
            i += 1;
            continue;
        }

        // Punctuation
        if matches!(chars[i], ';' | ',' | ':' | '_') {
            spans.push(TokenSpan::new(TokenKind::Punctuation, chars[i].to_string()));
            i += 1;
            continue;
        }

        // Dot (could be method call or float)
        if chars[i] == '.' {
            spans.push(TokenSpan::new(TokenKind::Punctuation, ".".to_owned()));
            i += 1;
            continue;
        }

        // Number literal
        if chars[i].is_ascii_digit()
            || (chars[i] == '-'
                && i + 1 < n
                && chars[i + 1].is_ascii_digit()
                && spans
                    .last()
                    .map(|s| !matches!(s.kind, TokenKind::Number | TokenKind::Plain))
                    .unwrap_or(true))
        {
            let mut s = String::new();
            if chars[i] == '-' {
                s.push('-');
                i += 1;
            }
            // hex prefix
            if i + 1 < n && chars[i] == '0' && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                s.push(chars[i]);
                s.push(chars[i + 1]);
                i += 2;
                while i < n && chars[i].is_ascii_hexdigit() {
                    s.push(chars[i]);
                    i += 1;
                }
            } else {
                while i < n && (chars[i].is_ascii_digit() || chars[i] == '_') {
                    s.push(chars[i]);
                    i += 1;
                }
                // decimal part
                if i + 1 < n && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
                    s.push('.');
                    i += 1;
                    while i < n && chars[i].is_ascii_digit() {
                        s.push(chars[i]);
                        i += 1;
                    }
                }
            }
            // optional suffix (u8, i32, f64, usize, etc.)
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                s.push(chars[i]);
                i += 1;
            }
            spans.push(TokenSpan::new(TokenKind::Number, s));
            continue;
        }

        // Identifier or keyword
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut s = String::new();
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                s.push(chars[i]);
                i += 1;
            }
            // Classify
            let kind = if s == "true" || s == "false" {
                TokenKind::Boolean
            } else if RUST_KEYWORDS.contains(&s.as_str()) {
                TokenKind::Keyword
            } else if RUST_TYPES.contains(&s.as_str()) {
                TokenKind::TypeName
            } else {
                TokenKind::Plain
            };
            spans.push(TokenSpan::new(kind, s));
            continue;
        }

        // Whitespace or any other character — accumulate as Plain
        {
            let mut s = String::new();
            while i < n
                && !chars[i].is_alphanumeric()
                && !matches!(
                    chars[i],
                    '(' | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '"'
                        | '\''
                        | '+'
                        | '-'
                        | '*'
                        | '/'
                        | '='
                        | '!'
                        | '&'
                        | '|'
                        | '<'
                        | '>'
                        | '^'
                        | '~'
                        | '@'
                        | '?'
                        | ';'
                        | ','
                        | ':'
                        | '_'
                        | '.'
                )
            {
                s.push(chars[i]);
                i += 1;
            }
            if !s.is_empty() {
                spans.push(TokenSpan::new(TokenKind::Plain, s));
            } else {
                // Single unhandled char (e.g. '#' not at line start in Rust)
                spans.push(TokenSpan::new(TokenKind::Plain, chars[i].to_string()));
                i += 1;
            }
        }
    }
    spans
}

// ─────────────────────────────────────────────────────────────────────────────
// Tokenizer: TOML
// ─────────────────────────────────────────────────────────────────────────────

/// Tokenizes one line of TOML source.
///
/// Returns a flat list of [`TokenSpan`]s whose texts concatenate to `line`.
#[must_use]
pub fn tokenize_toml_line(line: &str) -> Vec<TokenSpan> {
    let trimmed = line.trim_start();

    // Section header: `[...]` or `[[...]]`
    if trimmed.starts_with('[') {
        return vec![TokenSpan::new(TokenKind::Section, line.to_owned())];
    }

    // Comment line
    if trimmed.starts_with('#') {
        return vec![TokenSpan::new(TokenKind::Comment, line.to_owned())];
    }

    // Empty or pure-whitespace line
    if trimmed.is_empty() {
        return vec![TokenSpan::new(TokenKind::Plain, line.to_owned())];
    }

    // Key = value line: split on first '='
    if let Some(eq_pos) = line.find('=') {
        let key_part = &line[..eq_pos];
        let eq_char = "=";
        let value_part = &line[eq_pos + 1..];

        let mut spans: Vec<TokenSpan> = Vec::new();

        // Leading whitespace before key
        let key_trimmed = key_part.trim_end();
        let leading_ws = &key_part[..key_part.len() - key_trimmed.len()];
        if !leading_ws.is_empty() {
            spans.push(TokenSpan::new(TokenKind::Plain, leading_ws.to_owned()));
        }
        if !key_trimmed.is_empty() {
            spans.push(TokenSpan::new(TokenKind::Key, key_trimmed.to_owned()));
        }

        spans.push(TokenSpan::new(TokenKind::Operator, eq_char.to_owned()));

        // Tokenize the value side
        spans.extend(tokenize_toml_value(value_part));

        return spans;
    }

    // Fallback: plain line
    vec![TokenSpan::new(TokenKind::Plain, line.to_owned())]
}

/// Tokenizes the value side of a TOML `key = <value>` line.
fn tokenize_toml_value(value: &str) -> Vec<TokenSpan> {
    let mut spans: Vec<TokenSpan> = Vec::new();
    let chars: Vec<char> = value.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        // Comment
        if chars[i] == '#' {
            let rest: String = chars[i..].iter().collect();
            spans.push(TokenSpan::new(TokenKind::Comment, rest));
            break;
        }

        // Double-quoted string
        if chars[i] == '"' {
            let mut s = String::new();
            s.push('"');
            i += 1;
            while i < n {
                let c = chars[i];
                s.push(c);
                if c == '\\' && i + 1 < n {
                    i += 1;
                    s.push(chars[i]);
                } else if c == '"' {
                    break;
                }
                i += 1;
            }
            spans.push(TokenSpan::new(TokenKind::Str, s));
            i += 1;
            continue;
        }

        // Single-quoted string
        if chars[i] == '\'' {
            let mut s = String::new();
            s.push('\'');
            i += 1;
            while i < n && chars[i] != '\'' {
                s.push(chars[i]);
                i += 1;
            }
            if i < n {
                s.push('\'');
                i += 1;
            }
            spans.push(TokenSpan::new(TokenKind::Str, s));
            continue;
        }

        // Bracket characters
        if matches!(chars[i], '[' | ']' | '{' | '}') {
            spans.push(TokenSpan::new(TokenKind::Bracket, chars[i].to_string()));
            i += 1;
            continue;
        }

        // Comma / whitespace / other punctuation
        if matches!(chars[i], ',' | ' ' | '\t') {
            let mut s = String::new();
            while i < n && matches!(chars[i], ',' | ' ' | '\t') {
                s.push(chars[i]);
                i += 1;
            }
            spans.push(TokenSpan::new(TokenKind::Plain, s));
            continue;
        }

        // Number literal
        if chars[i].is_ascii_digit()
            || (chars[i] == '-' && i + 1 < n && chars[i + 1].is_ascii_digit())
        {
            let mut s = String::new();
            if chars[i] == '-' {
                s.push('-');
                i += 1;
            }
            while i < n && (chars[i].is_ascii_digit() || chars[i] == '_' || chars[i] == '.') {
                s.push(chars[i]);
                i += 1;
            }
            spans.push(TokenSpan::new(TokenKind::Number, s));
            continue;
        }

        // Keyword identifier: boolean, etc.
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let mut s = String::new();
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-') {
                s.push(chars[i]);
                i += 1;
            }
            let kind = if s == "true" || s == "false" {
                TokenKind::Boolean
            } else {
                TokenKind::Plain
            };
            spans.push(TokenSpan::new(kind, s));
            continue;
        }

        // Fallback: single char as Plain
        spans.push(TokenSpan::new(TokenKind::Plain, chars[i].to_string()));
        i += 1;
    }

    spans
}

// ─────────────────────────────────────────────────────────────────────────────
// Bracket matching
// ─────────────────────────────────────────────────────────────────────────────

fn matching_close(open: char) -> char {
    match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => open,
    }
}

fn matching_open(close: char) -> char {
    match close {
        ')' => '(',
        ']' => '[',
        '}' => '{',
        _ => close,
    }
}

/// Finds the matching bracket pair for the character at `(cursor_line,
/// cursor_col)` in the tokenized source.
///
/// - String and comment spans are skipped (no false matches inside literals).
/// - Returns `None` if the cursor is not adjacent to a bracket, if the token
///   kind at the cursor position is `Str` or `Comment`, or if no matching
///   bracket is found.
#[must_use]
pub fn find_bracket_match(
    tokens: &[Vec<TokenSpan>],
    cursor_line: usize,
    cursor_col: usize,
) -> Option<BracketPair> {
    // Build a flat view: (line_idx, char_col, char, skipped)
    // `skipped` = true when the char is inside a string or comment span.
    let mut flat: Vec<(usize, usize, char, bool)> = Vec::new();
    for (li, line_tokens) in tokens.iter().enumerate() {
        let mut col = 0usize;
        for span in line_tokens {
            let skip = matches!(span.kind, TokenKind::Str | TokenKind::Comment);
            for ch in span.text.chars() {
                flat.push((li, col, ch, skip));
                col += 1;
            }
        }
    }

    // Find the flat index corresponding to the cursor position.
    let cursor_idx = flat
        .iter()
        .position(|(l, c, _, _)| *l == cursor_line && *c == cursor_col)?;

    let (_l, _c, ch, skipped) = flat[cursor_idx];
    if skipped {
        return None;
    }

    match ch {
        '(' | '[' | '{' => {
            let close = matching_close(ch);
            let mut depth = 1usize;
            for item in flat.iter().skip(cursor_idx + 1) {
                let (l, c, c2, skip) = *item;
                if skip {
                    continue;
                }
                if c2 == ch {
                    depth += 1;
                } else if c2 == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(BracketPair {
                            open_line: cursor_line,
                            open_col: cursor_col,
                            close_line: l,
                            close_col: c,
                        });
                    }
                }
            }
            None
        }
        ')' | ']' | '}' => {
            let open = matching_open(ch);
            let mut depth = 1usize;
            for item in flat[..cursor_idx].iter().rev() {
                let (l, c, c2, skip) = *item;
                if skip {
                    continue;
                }
                if c2 == ch {
                    depth += 1;
                } else if c2 == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some(BracketPair {
                            open_line: l,
                            open_col: c,
                            close_line: cursor_line,
                            close_col: cursor_col,
                        });
                    }
                }
            }
            None
        }
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a zero-indexed char column to a byte offset within `s`.
///
/// Returns `s.len()` if `char_col >= s.chars().count()` (safe end-of-string
/// insertion point).
fn char_col_to_byte(s: &str, char_col: usize) -> usize {
    s.char_indices()
        .nth(char_col)
        .map(|(byte_idx, _)| byte_idx)
        .unwrap_or(s.len())
}

// ─────────────────────────────────────────────────────────────────────────────
// CodeEditor
// ─────────────────────────────────────────────────────────────────────────────

/// Stateful syntax-highlighted code editor view.
///
/// Supports editable and read-only modes, optional line-number gutter,
/// current-line highlight, bracket matching, and a basic autocomplete stub.
/// Language-specific tokenizing is provided for Rust and TOML.
///
/// # Headless tests
///
/// Use `apply_*` methods to drive state mutations from
/// [`gpui::TestAppContext`] without a `&mut Window`.
pub struct CodeEditor {
    /// Raw source text.
    source: String,
    /// Source split into individual lines (no trailing newlines).
    lines: Vec<String>,
    /// Active tokenizer language.
    language: EditorLanguage,
    /// When `true`, edit operations are silently ignored.
    read_only: bool,
    /// When `true`, a line-number gutter is rendered to the left of the code.
    show_line_numbers: bool,
    /// Maximum height of the editor in logical pixels.
    max_height: f32,
    /// Zero-indexed line of the cursor.
    cursor_line: usize,
    /// Zero-indexed char column of the cursor within the current line.
    cursor_col: usize,
    /// Tokenized lines — one `Vec<TokenSpan>` per line.
    tokens: Vec<Vec<TokenSpan>>,
    /// Matched bracket pair at the cursor position, if any.
    bracket_match: Option<BracketPair>,
    /// Whether the autocomplete suggestion overlay is currently visible.
    show_completions: bool,
    /// Dark-mode toggle.
    dark: bool,
    /// GPUI focus handle for keyboard routing.
    focus_handle: FocusHandle,
    /// Optional callback fired on every text change.
    on_change: Option<CodeChangeHandler>,
    /// Optional callback stub fired when autocomplete is triggered.
    on_complete: Option<CompletionHandler>,
}

impl CodeEditor {
    // ── construction ──────────────────────────────────────────────────────────

    /// Creates an empty Rust code editor in dark mode.
    ///
    /// Chain builder methods before passing to `cx.new`.
    #[must_use]
    pub fn new(cx: &mut Context<Self>) -> Self {
        let lines: Vec<String> = Vec::new();
        let tokens: Vec<Vec<TokenSpan>> = Vec::new();
        Self {
            source: String::new(),
            lines,
            language: EditorLanguage::Rust,
            read_only: false,
            show_line_numbers: true,
            max_height: 200.0,
            cursor_line: 0,
            cursor_col: 0,
            tokens,
            bracket_match: None,
            show_completions: false,
            dark: true,
            focus_handle: cx.focus_handle(),
            on_change: None,
            on_complete: None,
        }
    }

    // ── builder methods ───────────────────────────────────────────────────────

    /// Sets the initial source text.
    ///
    /// Splits into lines and tokenizes immediately.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self.lines = split_lines(&self.source);
        self.retokenize();
        self
    }

    /// Sets the tokenizer language.
    #[must_use]
    pub fn with_language(mut self, language: EditorLanguage) -> Self {
        self.language = language;
        self.retokenize();
        self
    }

    /// Sets read-only mode.  When `true`, all edit operations are no-ops.
    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Shows (`true`) or hides (`false`) the line-number gutter.
    #[must_use]
    pub fn show_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Sets the maximum height of the editor in logical pixels.
    #[must_use]
    pub fn with_max_height(mut self, height: f32) -> Self {
        self.max_height = height.max(40.0);
        self
    }

    /// Selects the dark (`true`) or light (`false`) token palette.
    #[must_use]
    pub fn dark(mut self, dark: bool) -> Self {
        self.dark = dark;
        self
    }

    /// Registers a callback invoked whenever the source text changes.
    ///
    /// Not called from headless `apply_*` methods — only from live GPUI
    /// keyboard event handlers.
    #[must_use]
    pub fn on_change(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Registers an autocomplete callback stub invoked when Ctrl+Space is
    /// pressed.  The callback receives the current source text.
    #[must_use]
    pub fn on_complete(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_complete = Some(Rc::new(handler));
        self
    }

    // ── runtime setters ───────────────────────────────────────────────────────

    /// Switches the dark/light theme at runtime and schedules a re-render.
    pub fn set_dark(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        cx.notify();
    }

    /// Replaces the source text at runtime, re-tokenizes, and notifies.
    pub fn set_source(&mut self, source: impl Into<String>, cx: &mut Context<Self>) {
        self.source = source.into();
        self.lines = split_lines(&self.source);
        self.cursor_line = self.cursor_line.min(self.lines.len().saturating_sub(1));
        let line_len = self
            .lines
            .get(self.cursor_line)
            .map(|l| l.chars().count())
            .unwrap_or(0);
        self.cursor_col = self.cursor_col.min(line_len);
        self.retokenize();
        self.update_bracket_match();
        cx.notify();
    }

    /// Switches the tokenizer language at runtime, re-tokenizes, and notifies.
    pub fn set_language(&mut self, language: EditorLanguage, cx: &mut Context<Self>) {
        self.language = language;
        self.retokenize();
        self.update_bracket_match();
        cx.notify();
    }

    // ── accessors ─────────────────────────────────────────────────────────────

    /// Returns the current source text.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the active language.
    #[must_use]
    pub fn language(&self) -> EditorLanguage {
        self.language
    }

    /// Returns `true` if the editor is in read-only mode.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Returns the number of lines in the source.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len().max(1)
    }

    /// Returns the zero-indexed line of the cursor.
    #[must_use]
    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }

    /// Returns the zero-indexed char column of the cursor within its line.
    #[must_use]
    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    /// Returns the token spans for a specific line, or an empty slice if the
    /// index is out of range.
    #[must_use]
    pub fn tokens_for_line(&self, line_idx: usize) -> &[TokenSpan] {
        self.tokens
            .get(line_idx)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Returns the matched bracket pair at the current cursor position, if any.
    #[must_use]
    pub fn bracket_match(&self) -> Option<BracketPair> {
        self.bracket_match
    }

    /// Returns `true` if the autocomplete overlay is currently shown.
    #[must_use]
    pub fn is_showing_completions(&self) -> bool {
        self.show_completions
    }

    /// Returns `true` if an `on_change` callback is registered.
    #[must_use]
    pub fn has_on_change(&self) -> bool {
        self.on_change.is_some()
    }

    /// Returns `true` if an `on_complete` callback is registered.
    #[must_use]
    pub fn has_on_complete(&self) -> bool {
        self.on_complete.is_some()
    }

    // ── headless edit mutators ────────────────────────────────────────────────

    /// Inserts `ch` at the current cursor position.
    ///
    /// Does nothing in read-only mode.  The `on_change` callback is **not**
    /// fired; it is only fired from the live GPUI event handler.
    pub fn apply_insert_char(&mut self, ch: char, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let line_text = self
            .lines
            .get(self.cursor_line)
            .cloned()
            .unwrap_or_default();
        let byte_at = char_col_to_byte(&line_text, self.cursor_col);
        let mut new_line = String::with_capacity(line_text.len() + ch.len_utf8());
        new_line.push_str(&line_text[..byte_at]);
        new_line.push(ch);
        new_line.push_str(&line_text[byte_at..]);
        self.set_line(self.cursor_line, new_line);
        self.cursor_col += 1;
        self.rebuild_source();
        self.retokenize();
        self.update_bracket_match();
        cx.notify();
    }

    /// Deletes the character immediately before the cursor (backspace).
    ///
    /// When the cursor is at column 0 and there is a previous line, merges the
    /// current line into the previous line.  Does nothing in read-only mode.
    pub fn apply_backspace(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if self.cursor_col > 0 {
            let line_text = self
                .lines
                .get(self.cursor_line)
                .cloned()
                .unwrap_or_default();
            let byte_before = char_col_to_byte(&line_text, self.cursor_col - 1);
            let byte_at = char_col_to_byte(&line_text, self.cursor_col);
            let mut new_line = String::with_capacity(line_text.len());
            new_line.push_str(&line_text[..byte_before]);
            new_line.push_str(&line_text[byte_at..]);
            self.set_line(self.cursor_line, new_line);
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            let prev_char_len = self
                .lines
                .get(self.cursor_line - 1)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            let prev = self
                .lines
                .get(self.cursor_line - 1)
                .cloned()
                .unwrap_or_default();
            let curr = self
                .lines
                .get(self.cursor_line)
                .cloned()
                .unwrap_or_default();
            let merged = prev + &curr;
            let removed_line = self.cursor_line;
            self.set_line(self.cursor_line - 1, merged);
            if removed_line < self.lines.len() {
                self.lines.remove(removed_line);
            }
            self.cursor_line -= 1;
            self.cursor_col = prev_char_len;
        }
        self.rebuild_source();
        self.retokenize();
        self.update_bracket_match();
        cx.notify();
    }

    /// Inserts a newline at the cursor, splitting the current line.
    ///
    /// Does nothing in read-only mode.
    pub fn apply_newline(&mut self, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let line_text = self
            .lines
            .get(self.cursor_line)
            .cloned()
            .unwrap_or_default();
        let byte_at = char_col_to_byte(&line_text, self.cursor_col);
        let before = line_text[..byte_at].to_owned();
        let after = line_text[byte_at..].to_owned();
        self.set_line(self.cursor_line, before);
        let insert_at = self.cursor_line + 1;
        if insert_at <= self.lines.len() {
            self.lines.insert(insert_at, after);
        } else {
            self.lines.push(after);
        }
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.rebuild_source();
        self.retokenize();
        self.update_bracket_match();
        cx.notify();
    }

    /// Moves the cursor to `(line, col)`, clamping both to valid bounds.
    pub fn apply_move_cursor(&mut self, line: usize, col: usize, cx: &mut Context<Self>) {
        let last_line = self.lines.len().saturating_sub(1);
        self.cursor_line = line.min(last_line);
        let line_len = self
            .lines
            .get(self.cursor_line)
            .map(|l| l.chars().count())
            .unwrap_or(0);
        self.cursor_col = col.min(line_len);
        self.update_bracket_match();
        cx.notify();
    }

    /// Shows the autocomplete overlay and fires the `on_complete` callback
    /// stub, if registered.
    ///
    /// The `on_complete` callback is **not** called from this headless helper
    /// (it requires a `&mut Window`); use the live GPUI event handler path
    /// for full callback dispatch.
    pub fn apply_trigger_completion(&mut self, cx: &mut Context<Self>) {
        self.show_completions = true;
        cx.notify();
    }

    /// Dismisses the autocomplete overlay.
    pub fn apply_dismiss_completion(&mut self, cx: &mut Context<Self>) {
        self.show_completions = false;
        cx.notify();
    }

    // ── internal helpers ──────────────────────────────────────────────────────

    /// Replaces the line at `idx`, extending the `lines` vec if necessary.
    fn set_line(&mut self, idx: usize, text: String) {
        if idx < self.lines.len() {
            self.lines[idx] = text;
        } else {
            while self.lines.len() < idx {
                self.lines.push(String::new());
            }
            self.lines.push(text);
        }
    }

    /// Rebuilds `self.source` from `self.lines`.
    fn rebuild_source(&mut self) {
        self.source = self.lines.join("\n");
    }

    /// Retokenizes all lines according to `self.language`.
    fn retokenize(&mut self) {
        self.tokens = self
            .lines
            .iter()
            .map(|line| match self.language {
                EditorLanguage::Rust => tokenize_rust_line(line),
                EditorLanguage::Toml => tokenize_toml_line(line),
            })
            .collect();
    }

    /// Recomputes `self.bracket_match` based on the current cursor position.
    fn update_bracket_match(&mut self) {
        self.bracket_match = find_bracket_match(&self.tokens, self.cursor_line, self.cursor_col);
    }

    /// Handles a key-down event in editable mode.
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;

        // Ctrl+Space → toggle autocomplete
        if (mods.control || mods.platform) && key == " " {
            self.show_completions = !self.show_completions;
            if self.show_completions
                && let Some(cb) = self.on_complete.clone()
            {
                cb(&self.source, window, cx);
            }
            cx.notify();
            return;
        }

        // Escape → dismiss completion
        if key == "escape" {
            self.show_completions = false;
            cx.notify();
            return;
        }

        // Navigation keys — no Ctrl/Cmd modifier
        if !mods.control && !mods.platform {
            match key {
                "backspace" => {
                    self.apply_backspace(cx);
                    if let Some(cb) = self.on_change.clone() {
                        cb(&self.source, window, cx);
                    }
                    return;
                }
                "return" => {
                    self.apply_newline(cx);
                    if let Some(cb) = self.on_change.clone() {
                        cb(&self.source, window, cx);
                    }
                    return;
                }
                "up" => {
                    if self.cursor_line > 0 {
                        let new_line = self.cursor_line - 1;
                        let len = self
                            .lines
                            .get(new_line)
                            .map(|l| l.chars().count())
                            .unwrap_or(0);
                        self.cursor_line = new_line;
                        self.cursor_col = self.cursor_col.min(len);
                        self.update_bracket_match();
                        cx.notify();
                    }
                    return;
                }
                "down" => {
                    if self.cursor_line + 1 < self.lines.len() {
                        let new_line = self.cursor_line + 1;
                        let len = self
                            .lines
                            .get(new_line)
                            .map(|l| l.chars().count())
                            .unwrap_or(0);
                        self.cursor_line = new_line;
                        self.cursor_col = self.cursor_col.min(len);
                        self.update_bracket_match();
                        cx.notify();
                    }
                    return;
                }
                "left" => {
                    if self.cursor_col > 0 {
                        self.cursor_col -= 1;
                        self.update_bracket_match();
                        cx.notify();
                    } else if self.cursor_line > 0 {
                        self.cursor_line -= 1;
                        self.cursor_col = self
                            .lines
                            .get(self.cursor_line)
                            .map(|l| l.chars().count())
                            .unwrap_or(0);
                        self.update_bracket_match();
                        cx.notify();
                    }
                    return;
                }
                "right" => {
                    let line_len = self
                        .lines
                        .get(self.cursor_line)
                        .map(|l| l.chars().count())
                        .unwrap_or(0);
                    if self.cursor_col < line_len {
                        self.cursor_col += 1;
                        self.update_bracket_match();
                        cx.notify();
                    } else if self.cursor_line + 1 < self.lines.len() {
                        self.cursor_line += 1;
                        self.cursor_col = 0;
                        self.update_bracket_match();
                        cx.notify();
                    }
                    return;
                }
                _ => {}
            }
        }

        // Printable character
        if let Some(ch_str) = &event.keystroke.key_char
            && !mods.control
            && !mods.platform
            && !mods.alt
            && let Some(ch) = ch_str.chars().next()
        {
            self.apply_insert_char(ch, cx);
            if let Some(cb) = self.on_change.clone() {
                cb(&self.source, window, cx);
            }
        }
    }
}

/// Splits `source` into a `Vec<String>` of lines without trailing newlines.
///
/// An empty string produces `vec![""]` (one empty line).
fn split_lines(source: &str) -> Vec<String> {
    if source.is_empty() {
        return vec![String::new()];
    }
    source.lines().map(str::to_owned).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Focusable
// ─────────────────────────────────────────────────────────────────────────────

impl Focusable for CodeEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Render
// ─────────────────────────────────────────────────────────────────────────────

/// Width of the line-number gutter in logical pixels.
const GUTTER_WIDTH: f32 = 40.0;
/// Font size used throughout the editor.
const EDITOR_FONT_SIZE: f32 = 13.0;
/// Line height in logical pixels.
const LINE_HEIGHT: f32 = 20.0;

impl Render for CodeEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = if self.dark {
            Theme::from(&*ThemeColor::dark())
        } else {
            Theme::from(&*ThemeColor::light())
        };
        let colors = CodeEditorColors::resolve(&theme, self.dark);
        let focus = self.focus_handle.clone();
        let entity = cx.entity();

        // Snapshot data needed inside closures.
        let tokens_snapshot: Vec<Vec<TokenSpan>> = self.tokens.clone();
        let bracket_match = self.bracket_match;
        let cursor_line = self.cursor_line;
        let show_line_numbers = self.show_line_numbers;
        let show_completions = self.show_completions;
        let language = self.language;
        let read_only = self.read_only;
        let max_height = self.max_height;

        // ── Header bar ────────────────────────────────────────────────────────

        let mode_label: SharedString = if read_only {
            SharedString::from(format!("{} · read-only", language.label()))
        } else {
            SharedString::from(format!("{} · editable", language.label()))
        };

        let header = div()
            .id("ce-header")
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_2()
            .h(px(22.0))
            .bg(colors.header_bg)
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.header_text)
                    .child(mode_label),
            );

        // ── Content area ──────────────────────────────────────────────────────

        let content_height = (max_height - 22.0).max(40.0);

        // Content rows (gutter + code)
        let mut lines_col = div()
            .id("ce-lines")
            .flex()
            .flex_col()
            .flex_grow(1.0)
            .h(px(content_height))
            .overflow_hidden()
            .font_family(crate::MONO_FONT_FAMILY);

        let line_count = tokens_snapshot.len().max(1);

        for (line_idx, line_tokens) in tokens_snapshot.iter().enumerate() {
            let is_current = line_idx == cursor_line;
            let line_bg = if is_current {
                colors.current_line_bg
            } else {
                colors.background
            };

            // Gutter cell
            let gutter_cell = if show_line_numbers {
                let line_num_text: SharedString = SharedString::from((line_idx + 1).to_string());
                Some(
                    div()
                        .flex_shrink_0()
                        .w(px(GUTTER_WIDTH))
                        .h(px(LINE_HEIGHT))
                        .flex()
                        .flex_row()
                        .justify_end()
                        .items_center()
                        .pr_2()
                        .bg(colors.gutter_bg)
                        .text_size(px(EDITOR_FONT_SIZE - 1.0))
                        .text_color(if is_current {
                            colors.text
                        } else {
                            colors.gutter_text
                        })
                        .child(line_num_text),
                )
            } else {
                None
            };

            // Token spans for this line
            let mut token_row = div()
                .flex()
                .flex_row()
                .items_center()
                .flex_grow(1.0)
                .h(px(LINE_HEIGHT))
                .px_2()
                .bg(line_bg)
                .text_size(px(EDITOR_FONT_SIZE));

            // Render each token span with its syntax color.
            for (span_idx, span) in line_tokens.iter().enumerate() {
                // Check if this span is one of the bracket match characters.
                // A match is detected if the span is a single bracket char whose
                // (line, col) matches open or close of the bracket_match pair.
                let span_col = {
                    let mut col = 0usize;
                    for prev in &line_tokens[..span_idx] {
                        col += prev.text.chars().count();
                    }
                    col
                };

                let is_bracket_match = bracket_match
                    .map(|bp| {
                        (line_idx == bp.open_line && span_col == bp.open_col)
                            || (line_idx == bp.close_line && span_col == bp.close_col)
                    })
                    .unwrap_or(false);

                let text_color = colors.token_color(span.kind);
                let span_text: SharedString = SharedString::from(span.text.clone());

                let span_div = if is_bracket_match {
                    div()
                        .text_color(text_color)
                        .bg(colors.bracket_match_bg)
                        .child(span_text)
                } else {
                    div().text_color(text_color).child(span_text)
                };

                token_row = token_row.child(span_div);
            }

            // Empty line: add a cursor indicator in editable mode.
            if line_tokens.is_empty() && is_current && !read_only {
                token_row =
                    token_row.child(div().w(px(2.0)).h(px(LINE_HEIGHT - 2.0)).bg(colors.text));
            }

            // Build the full line row with gutter + code, with click-to-focus.
            let e_click = entity.clone();
            let li = line_idx;
            let mut line_row = div()
                .id(ElementId::Integer(200_000 + line_idx as u64))
                .flex()
                .flex_row()
                .flex_shrink_0()
                .w_full()
                .h(px(LINE_HEIGHT))
                .cursor_pointer()
                .on_click(move |_, _win, app| {
                    e_click.update(app, |editor, cx| {
                        editor.apply_move_cursor(li, 0, cx);
                    });
                });

            if let Some(g) = gutter_cell {
                line_row = line_row.child(g);
            }
            line_row = line_row.child(token_row);
            lines_col = lines_col.child(line_row);
        }

        // Placeholder for empty editor
        if line_count == 1 && self.lines.first().map(String::is_empty).unwrap_or(true) {
            lines_col = lines_col.child(
                div()
                    .px_2()
                    .text_size(px(EDITOR_FONT_SIZE))
                    .text_color(colors.gutter_text)
                    .child(SharedString::from("(empty)")),
            );
        }

        // ── Autocomplete overlay ──────────────────────────────────────────────

        let completion_stubs: &[&str] = match language {
            EditorLanguage::Rust => &[
                "fn ",
                "let ",
                "pub struct ",
                "impl ",
                "use std::",
                "Vec::new()",
            ],
            EditorLanguage::Toml => &[
                "[dependencies]",
                "[package]",
                "version = \"\"",
                "edition = \"2024\"",
            ],
        };

        let completion_overlay = if show_completions {
            let mut overlay = div()
                .id("ce-completions")
                .mt_1()
                .mx_2()
                .mb_1()
                .rounded(px(4.0))
                .border_1()
                .border_color(colors.completion_border)
                .bg(colors.completion_bg)
                .overflow_hidden();

            for (idx, stub) in completion_stubs.iter().enumerate() {
                let stub_text: SharedString = SharedString::from(*stub);
                overlay = overlay.child(
                    div()
                        .id(ElementId::Integer(300_000 + idx as u64))
                        .px_2()
                        .h(px(LINE_HEIGHT))
                        .flex()
                        .flex_row()
                        .items_center()
                        .text_size(px(EDITOR_FONT_SIZE))
                        .font_family(crate::MONO_FONT_FAMILY)
                        .text_color(colors.completion_text)
                        .cursor_pointer()
                        .child(stub_text),
                );
            }

            Some(overlay)
        } else {
            None
        };

        // ── Outer container ───────────────────────────────────────────────────

        let mut outer = div()
            .id("code-editor")
            .track_focus(&focus)
            .key_context("CodeEditor")
            .flex()
            .flex_col()
            .rounded(px(RadiusScale::default().md))
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .max_h(px(max_height + 22.0))
            .overflow_hidden()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .child(header)
            .child(lines_col);

        if let Some(overlay) = completion_overlay {
            outer = outer.child(overlay);
        }

        outer
    }
}
