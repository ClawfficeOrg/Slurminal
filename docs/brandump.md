# Brandump — Slurminal + Specttyr + zoid-slurminal-component

Captured 2026-09-16 from a live jam session. This is the raw intent, not a spec.
It exists so the idea survives tab switches. Turn it into TaskerKeeper plans in
each repo; keep this file as the origin record.

---

## Projects

| Name | Repo | What it is |
|---|---|---|
| **zoid-slurminal-component** | in Zoid (`crates/zoid-slurminal-component`) | libghostty-vt bindings + GPUI terminal surface. The reusable crate. |
| **Slurminal** | `git@github.com:ClawfficeOrg/Slurminal.git` | The terminal app built on the component. |
| **Specttyr** | `git@github.com:ClawfficeOrg/Specttyr.git` | Rust/GPUI rewrite of Paseo. |

Both apps scaffold from Zoid templates (menubar/taskbar) and land as submodules
in Zoid's `examples/` directory. Split-out of the component into bindings-only +
surface-only crates is a later option, not a now-decision.

---

## Slurminal — terminal app

Inspiration targets: WezTerm (feature parity goal), Ghostty (emulator core),
Kitty (graphics protocols).

### Feature list (raw)

- **Basic terminal**: PTY, VT parsing, scrollback, selection, search, links.
- **Nerd Font support**: glyph fallback / font feature detection, not just
  "hope the font has it".
- **Background images**: per-pane and/or per-window, with opacity/blur/cover
  modes.
- **Quake-style drop-down mode**: hotkey summon, slide animation, per-monitor
  placement, "wait until you hear these ideas" — TBD, expand into a dedicated
  design doc.
- **WezTerm extras**: the long tail. Enumerate WezTerm's feature surface and
  decide include/exclude per item rather than hand-waving "all of it".
- **Highly configurable, with a UI**: every setting reachable from a settings
  GUI, not just a config file. Config file still exists; UI is a first-class
  editor of it.
- **Kitty image protocol** support.
- **Sixel** support.
- **Ratatui-based console app** for Slurminal (TUI companion / headless mode).
- **Figby support, built in.** Figby is a Rust port of FIGlet 2.2.5 (`figby`
  crate, v6.0.33, `crate-type = ["cdylib", "lib"]`, on crates.io, BSD-3-Clause,
  source at `~/git_repos/Figby` in WSL / github.com/DoseOfGose/figby). It is a
  FIGfont/TOIlet banner renderer that grew a full-screen ratatui TUI editor with
  drawing tools, layers, a palette editor, a font editor, image import, and an
  animation timeline with keyframing, tweening, onion skinning, and GIF/APNG/ANSI
  export. It also has `--play <file.gif>` for fullscreen terminal GIF playback.
  Intent: use it to make Slurminal's TUI flashy and extravagant while the
  terminal itself is being built. It renders through ratatui, which is the same
  stack as the planned console companion, so it fits there naturally. Licence is
  BSD-3-Clause — permissive, so attribution is all that is required; ship it with
  the other bundled licences.

### Open questions to resolve in the Slurminal plan

- Quake mode specifics (the "wait until you hear these ideas" part).
- Which WezTerm features are in-scope for v1 vs deferred.
- Config format + schema + UI binding story.
- Graphics protocols: which are v1, which are later.
- How far to take Figby integration: use the crate's render API for banners, or
  adopt its TUI/animation surface more deeply.


---

## Specttyr — Paseo Rust rewrite

- **Relationship to Paseo**: port the protocol to Rust serde types, speak the
  same wire format, so Specttyr can be tested against a real Paseo daemon
  immediately. **Split from Paseo is a future direction** — diverge later where
  Rust/GPUI makes another shape obviously better. Do not over-fit to Paseo
  internals now.
- **Everything else**: TBD from the Paseo walkthrough (workspaces, agents,
  timeline, git panels, composer, terminal panes).

---

## Cross-cutting

- Both apps are Zoid examples first-class: they prove Zoid can build real
  applications, and they drive component requirements back into `zoid_gpui`.
- Slurminal depends on `zoid-slurminal-component`. Specttyr depends on Slurminal
  (or at least the component) for its terminal panes.
- Windows is a first-class target. libghostty-vt already supports Windows; a
  Rust + GPUI Windows terminal surface is the novel part.
