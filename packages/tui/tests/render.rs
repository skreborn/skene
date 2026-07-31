//! Headless renders of the terminal driver, driven through the raw vocabulary.

#![cfg(not(target_arch = "wasm32"))]

use skene_core::{
  Axis,
  Border,
  BoxProps,
  BoxStyle,
  Color,
  Condition,
  InputProps,
  Layout,
  RunDisplay,
  RunProps,
  RunStyle,
};
use skene_tui as tui;

/// Serializes the tests: the UI-thread designation is process-wide, so each test must
/// install, run, and uninstall without another test's thread interleaving.
static DRIVER: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Installs a fresh driver, runs `build`, and uninstalls afterwards.
fn with_driver<R>(build: impl FnOnce() -> R) -> R {
  let _serialized = DRIVER
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner);

  tui::install();

  let result = build();

  tui::uninstall();

  result
}

fn plain_box(layout: Layout, style: BoxStyle) -> skene_core::NodeId {
  tui::create_box(&BoxProps {
    layout,
    role: None,
    style,
    variants: Vec::new(),
    transitions: Vec::new(),
    animations: Vec::new(),
  })
}

fn text_run(text: &str) -> skene_core::NodeId {
  let node = tui::create_run(&RunProps {
    display: RunDisplay::Block,
    link: None,
    style: RunStyle::new(),
    variants: Vec::new(),
    transitions: Vec::new(),
    animations: Vec::new(),
  });

  tui::set_text(node, text);

  node
}

/// A column lays its runs on successive rows.
#[test]
fn column_stacks_runs() {
  let rendered = with_driver(|| {
    let root = plain_box(
      Layout::Flex {
        direction: Axis::Vertical,
      },
      BoxStyle::new(),
    );

    tui::attach(root, text_run("alpha"), None);
    tui::attach(root, text_run("beta"), None);
    tui::mount(|| root);

    tui::render_to_text(12, 4)
  });

  assert_eq!(rendered, "alpha\nbeta\n\n\n");
}

/// A row lays its runs side by side, separated by the gap.
#[test]
fn row_flows_runs_with_spacing() {
  let rendered = with_driver(|| {
    let root = plain_box(
      Layout::Flex {
        direction: Axis::Horizontal,
      },
      BoxStyle::new().set_spacing(2.),
    );

    tui::attach(root, text_run("ab"), None);
    tui::attach(root, text_run("cd"), None);
    tui::mount(|| root);

    tui::render_to_text(12, 2)
  });

  assert_eq!(rendered, "ab  cd\n\n");
}

/// A border draws box-drawing characters around padded content.
#[test]
fn border_draws_a_frame() {
  let rendered = with_driver(|| {
    let root = plain_box(
      Layout::Block,
      BoxStyle::new()
        .set_width(7.)
        .set_height(3.)
        .set_border(Border::new(1., Color::srgb(1., 1., 1.))),
    );

    tui::attach(root, text_run("hi"), None);
    tui::mount(|| root);

    tui::render_to_text(10, 4)
  });

  // The root child fills the viewport (as the web reset's `body > *` does), so the
  // 3-tall box stretches to the 4-row frame.
  assert_eq!(rendered, "┌─────┐\n│hi   │\n│     │\n└─────┘\n");
}

/// A `MaxWidth` variant applies at narrow terminal sizes and not at wide ones.
#[test]
fn media_variant_resolves_against_terminal_size() {
  let (narrow, wide) = with_driver(|| {
    let root = tui::create_box(&BoxProps {
      layout: Layout::Block,
      role: None,
      style: BoxStyle::new().set_width(6.).set_height(3.),
      variants: vec![(
        vec![Condition::MaxWidth(10.)],
        BoxStyle::new().set_border(Border::new(1., Color::srgb(1., 1., 1.))),
      )],
      transitions: Vec::new(),
      animations: Vec::new(),
    });

    tui::mount(|| root);

    (tui::render_to_text(8, 4), tui::render_to_text(20, 4))
  });

  assert!(narrow.contains('┌'));
  assert!(!wide.contains('┌'));
}

/// An input shows its placeholder dimmed until it has a value.
#[test]
fn input_shows_placeholder_then_value() {
  let rendered = with_driver(|| {
    let input = tui::create_input(&InputProps {
      placeholder: Some("name".to_owned()),
      style: BoxStyle::new(),
      variants: Vec::new(),
      transitions: Vec::new(),
      animations: Vec::new(),
    });

    tui::mount(|| input);

    let before = tui::render_to_text(10, 2);

    tui::set_input_value(input, "Ada");

    let after = tui::render_to_text(10, 2);

    (before, after)
  });

  assert_eq!(rendered.0, "name\n\n");
  assert_eq!(rendered.1, "Ada\n\n");
}

/// Detaching and destroying removes a subtree from the next frame.
#[test]
fn destroy_removes_a_subtree() {
  let rendered = with_driver(|| {
    let root = plain_box(
      Layout::Flex {
        direction: Axis::Vertical,
      },
      BoxStyle::new(),
    );

    let gone = text_run("gone");

    tui::attach(root, text_run("kept"), None);
    tui::attach(root, gone, None);
    tui::mount(|| root);
    tui::detach(root, gone);
    tui::destroy(gone);

    tui::render_to_text(8, 3)
  });

  assert_eq!(rendered, "kept\n\n\n");
}
