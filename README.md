# Slurminal

A Rust and GPUI reimplementation of the Paseo client. Ports the Paseo wire
protocol to typed serde structs and speaks the same WebSocket format, so Slurminal
works against a real Paseo daemon from the first working build.

Scaffolded from Zoid's `menubar` GPUI template. It is a first-class Zoid example,
and it is the second real consumer of `zoid-terminal` for its terminal
panes.

## Status

Planning. See [`docs/todo-v1.json`](docs/todo-v1.json) for the implementation plan.

**Stance:** Paseo-compatible now, diverge later. Compatibility is a feature until
it is a constraint; divergence is reviewed at v2, not during v1. The Paseo
checkout is the protocol authority — `packages/protocol` for the wire format,
`packages/app` for UI reference.

## Planning

```sh
taskerkeeper list docs/todo-v1.json
taskerkeeper ready docs/todo-v1.json --json
```

## Template Purpose

The application menu provides primary navigation (app menu, File, View), with
Preferences and a light/dark theme toggle reachable from the menu. Quit
dispatches the `Quit` action, which calls `App::quit()`.

## Commands

```sh
cargo run
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Structure

| Module            | Purpose                                                         |
|-------------------|-----------------------------------------------------------------|
| `main.rs`         | App entry point: menu install, window creation, quit handler    |
| `menu_actions.rs` | GPUI actions, keybindings, and both menu trees (native + in-window) |
| `menu_bar.rs`     | In-window menu bar drawn by the app (Windows and Linux only)    |
| `settings.rs`     | Typed app settings with safe load/save                         |
| `ui/`             | Generated from `ui/app.json` by `zoid export` — do not hand-edit |

## Platform Notes

The menu works on every platform, but it is built two ways because GPUI's
`App::set_menus()` only reaches an OS menu API on macOS.

- **macOS**: `set_menus()` installs the native global menu bar from
  `menu_actions::build_menus()`, including the `Services` system submenu.
- **Windows / Linux**: the app draws its own menu bar at the top of the window
  (`menu_bar.rs`) from `menu_actions::build_in_window_menus()`. `F10` opens the
  first menu; left/right arrows move between menus. All other keybindings are the
  same on every platform.
- Keep the two menu trees in sync when you add items: `build_menus()` for macOS,
  `build_in_window_menus()` for Windows and Linux.

## GPUI Baseline

- GPUI: tracks Zed `main`. This project declares gpui as a bare git dependency,
  so the first `cargo build` resolves whatever Zed `main` is that day and writes
  it to your `Cargo.lock`. Commit that lock — it is what makes your build
  reproducible. `cargo update` is then a deliberate act, not a side effect.
- Last verified by Zoid against: `gpui-kit 0.6.1 (gpui-pre 0.3.5)`
- Source checked by Zoid: docs.rs `gpui/latest`, gpui.rs examples, and Zed `main` `crates/gpui`.

GPUI is pre-1.0. Re-check current GPUI docs before changing app lifecycle, actions,
menus, or window management.
