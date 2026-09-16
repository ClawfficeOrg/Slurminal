//! Compile-checked fixtures for Zoid's code generators.
//!
//! Each file here is the **verbatim** output of an emitter in
//! `zoid_template::zml_codegen`, checked in so that `cargo build` type-checks
//! the generated code against the real `gpui` and `gpui-component` APIs. A
//! snapshot test in `zoid_template` asserts the emitter still produces these
//! bytes exactly.
//!
//! That pairing is what makes the emitters safe to change: a snapshot test
//! alone would happily accept a renamed `WindowOptions` field, and a compile
//! check alone would not notice the emitter drifting away from the fixture.
//! GPUI is pre-1.0 and its window API moves, so both halves are needed.
//!
//! Do not hand-edit these files — regenerate them from the emitter.

// The fixtures are verbatim emitter output: they carry no module docs, their
// `pub(super)` items are never called (compiling them is the whole point), and
// their formatting is the emitter's, not rustfmt's.  The emitter also writes a
// fixed import header whatever the document uses, and clones item fields whose
// `Copy`-ness it cannot know — both are deliberate, and both are lints in a
// generated project rather than errors, so they are allowed here too.
#[allow(dead_code, missing_docs, unused_imports, clippy::clone_on_copy)]
#[rustfmt::skip]
pub mod window_options;

#[allow(dead_code, missing_docs, unused_imports, clippy::clone_on_copy)]
#[rustfmt::skip]
pub mod zml_layout;

// The `Repeat` emitter is a second `zml_layout` shape — iterated children, a
// per-row element id, and item-scoped binding refs — so it gets its own
// fixture rather than being folded into the flat one (todo-v8 8.15.5).
#[allow(dead_code, missing_docs, unused_imports, clippy::clone_on_copy)]
#[rustfmt::skip]
pub mod zml_layout_repeat;

// Not generated: the host-side row types the repeat fixture's bindings name.
pub mod repeat_support;
