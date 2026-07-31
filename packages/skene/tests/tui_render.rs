//! The platform-neutrality proof: the same portable views that render to the DOM and to
//! HTML render to a terminal text grid, unmodified.
//!
//! Run with `cargo test -p skene --features tui`.

#![cfg(all(not(target_arch = "wasm32"), feature = "tui"))]

use std::{cell::Cell, rc::Rc};

use skene::{
  Border,
  Container,
  HasChildren,
  Input,
  Inset,
  Text,
  View,
  core::{BoxStyle, Color, Condition},
  reactive::{RwSignal, SignalGet, SignalUpdate},
  tui,
};

/// Serializes the tests: the UI-thread designation is process-wide, so each test must
/// install, run, and uninstall without another test's thread interleaving.
static DRIVER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Installs a fresh driver, builds `view`, and returns the frame at `width` x `height`.
fn render(width: u16, height: u16, view: impl FnOnce() -> skene::AnyView) -> String {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();
  tui::mount(|| view().build());

  let text = tui::render_to_text(width, height);

  tui::uninstall();

  text
}

/// The counter-style composition renders: a bordered column with text rows.
#[test]
fn portable_views_render_to_the_terminal() {
  let text = render(14, 7, || {
    Container::column()
      .set_border(Border::new(1., Color::srgb(1., 1., 1.)))
      .set_padding(Inset::all(1.))
      .set_width(12.)
      .set_height(6.)
      .set_children((Text::new("Count: 3"), Text::new("ready")))
      .into_any()
  });

  assert_eq!(
    text,
    "┌──────────┐\n│          │\n│ Count: 3 │\n│ ready    │\n│          │\n│          │\n└──────────┘\n",
  );
}

/// A derived text re-renders when its signal changes, through the same reactive path as
/// every other driver.
#[test]
fn derived_text_updates_the_frame() {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let count = RwSignal::new(1u32);

  tui::mount(|| {
    Text::derive(move || format!("n={}", count.get()))
      .into_any()
      .build()
  });

  let before = tui::render_to_text(8, 1);

  count.set(2);

  let after = tui::render_to_text(8, 1);

  tui::uninstall();

  assert_eq!(before, "n=1\n");
  assert_eq!(after, "n=2\n");
}

/// A focus-conditioned variant applies when the driver focuses the node.
#[test]
fn focus_variant_applies_on_tab() {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let clicks = Rc::new(Cell::new(0u32));

  {
    let clicks = Rc::clone(&clicks);

    tui::mount(|| {
      Container::new()
        .set_width(6.)
        .set_height(3.)
        .set_when(
          Condition::Focused,
          BoxStyle::new().set_border(Border::new(1., Color::srgb(1., 1., 1.))),
        )
        .handle_click(move || clicks.set(clicks.get() + 1))
        .into_any()
        .build()
    });
  }

  let unfocused = tui::render_to_text(8, 4);

  tui::trigger_key("Tab");

  let focused = tui::render_to_text(8, 4);

  tui::trigger_key("Enter");

  let count = clicks.get();

  tui::uninstall();

  assert!(!unfocused.contains('┌'));
  assert!(focused.contains('┌'));
  assert_eq!(count, 1);
}

/// Typing into a focused input flows through `bind_value` into the signal and back onto
/// the frame.
#[test]
fn input_binds_a_signal_both_ways() {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let name = RwSignal::new(String::new());

  tui::mount(|| {
    Input::new()
      .set_placeholder("name")
      .bind_value(name)
      .into_any()
      .build()
  });

  tui::trigger_key("Tab");
  tui::trigger_key("h");
  tui::trigger_key("i");

  let value = name.get_untracked();
  let frame = tui::render_to_text(10, 1);

  tui::uninstall();

  assert_eq!(value, "hi");
  assert!(frame.starts_with("hi"));
}
