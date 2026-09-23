# Changelog

All notable changes to Slurminal will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Initial scaffold from Zoid's `menubar` template, plus the v1 plan.
- A real terminal (Zoid task 9.3.5): the default shell runs in a PTY and
  fills the window under the menu bar. Built only on
  `zoid-slurminal-component`'s public interface: `PtySession` +
  `pump_to_channel` for the process, `TerminalView::feed` for output (it
  also answers VT queries such as ConPTY's startup cursor report),
  `TerminalEvent::Input` for keys and query replies, `TerminalView::focus`
  for keyboard focus. The shell starts when the window opens; the window
  closes when the shell exits; closing the window kills and reaps it.

### Changed

- Vendored crates refreshed from Zoid `release/9.0` + `task-9.3.5`, adding
  the terminal chain (`zoid-slurminal-component`, `zoid-ghostty-vt`,
  `zoid-ghostty-vt-sys`) and `zoid-gpui`'s `terminal` feature. Migration
  for a user project: re-vendor (`zoid check --update-vendor`, or the same
  copies by hand), add the component path dependency, add a `[workspace]`
  table if the project sits inside another workspace. Zig + network are
  now needed on first build.

### Known gaps

- Fixed 80x24 grid; the window does not resize the terminal yet.
- App shortcuts (`Ctrl+W`, `Ctrl+N`, `Ctrl+Q`, `F10`) take precedence over
  the shell.
- No IME, mouse, paste, selection, or scrollback wheel yet.
