//! The window shell over the seam: a tree built as the facade builds it, laid out and
//! painted into the shell's own frame, then hovered, pressed, clicked and tabbed through.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{
  Color, State,
  seam::{
    content::contents,
    node::{ControlKind, LayoutKind, NodeKind, NodeSpec},
    record::{Record, keys},
    tree,
  },
  value::When as _,
};
use skene_gui_linux::{Linux, keys::KeyInput, shell::Shell};

use super::{boxed, node, owned};

const REST: Color = Color::rgb(0xffffff);
const HOVER: Color = Color::rgb(0xdbeafe);
const PRESSED: Color = Color::rgb(0x93c5fd);

fn pixel(shell: &Shell, x: u32, y: u32) -> Color {
  let frame = shell.last_frame().expect("a frame was painted");
  let pixel = frame.data()[(y * u32::from(frame.width()) + x) as usize];

  Color {
    red: pixel.r,
    green: pixel.g,
    blue: pixel.b,
    alpha: pixel.a,
  }
}

fn key(name: &str, down: bool) -> KeyInput {
  KeyInput {
    event: skene_foundation::seam::report::KeyEvent {
      key: name.to_owned(),
      code: String::new(),
      down,
      repeat: false,
      modifiers: Default::default(),
    },
    text: None,
    keycode: 0,
    composing: None,
    composed: false,
  }
}

#[test]
fn a_row_restyles_under_hover_and_press_and_activates_on_click_outside_its_switch() {
  owned(|| {
    let mut record = Record::new();

    record
      .set(
        keys::BACKGROUND,
        REST
          .when(State::Hovered, HOVER)
          .when(State::Pressed, PRESSED),
      )
      .set(keys::HEIGHT, skene_foundation::Dimension::Length(60.));

    let (root, root_node) = boxed(Record::new());
    let (row, _) = node(
      NodeSpec::new(NodeKind::Layout(LayoutKind::Box)).acting(),
      record,
    );
    let mut centered = Record::new();

    centered.set(keys::SELF_ALIGNMENT, skene_foundation::Alignment::CENTER);

    let (switch, _) = node(
      NodeSpec::new(NodeKind::Control(ControlKind::Switch)),
      centered,
    );
    let activations = Rc::new(Cell::new(0));
    let on = skene_foundation::Signal::new(false);

    {
      let activations = Rc::clone(&activations);

      tree::handle_activate::<Linux>(row, move || activations.set(activations.get() + 1));
    }

    tree::control::<Linux, _>(
      switch,
      contents::ON,
      move || on.get(),
      Some(Box::new(move |value| on.set(value))),
    );
    tree::attach::<Linux>(root, row, 0);
    tree::attach::<Linux>(row, switch, 0);

    let mut shell = Shell::new("test", 200, 100);

    shell.set_root(root_node);
    shell.configure(200, 100);
    assert!(shell.frame().is_some(), "the first frame paints");
    assert_eq!(pixel(&shell, 10, 30), REST);

    shell.pointer_motion(10., 30.);
    shell.frame();
    assert!(tree::holds::<Linux>(row, State::Hovered));
    assert_eq!(pixel(&shell, 10, 30), HOVER);

    shell.pointer_button(0, true, (10., 30.));
    shell.frame();
    assert_eq!(pixel(&shell, 10, 30), PRESSED);

    shell.pointer_button(0, false, (10., 30.));
    shell.frame();
    assert_eq!(activations.get(), 1, "a click on the row activates it");
    assert_eq!(pixel(&shell, 10, 30), HOVER);

    // The switch is centered in the row: a click on it flips it alone.
    shell.pointer_motion(100., 30.);
    shell.pointer_button(0, true, (100., 30.));
    shell.pointer_button(0, false, (100., 30.));
    assert!(on.get_untracked(), "the switch owns its press");
    assert_eq!(activations.get(), 1, "the row did not activate");

    shell.pointer_leave();
    shell.frame();
    assert!(!tree::holds::<Linux>(row, State::Hovered));
    assert_eq!(pixel(&shell, 10, 30), REST);
  });
}

#[test]
fn tab_follows_the_scenes_child_order_and_enter_activates_a_row() {
  owned(|| {
    let (root, root_node) = boxed(Record::new());
    let mut column = Record::new();

    column.set(keys::SPACING, skene_foundation::Spacing::from(4.));

    let (list, _) = node(NodeSpec::new(NodeKind::Layout(LayoutKind::Column)), column);
    let rows: Vec<_> = (0..3)
      .map(|_| {
        let mut record = Record::new();

        record.set(keys::HEIGHT, skene_foundation::Dimension::Length(20.));
        node(
          NodeSpec::new(NodeKind::Layout(LayoutKind::Box)).acting(),
          record,
        )
        .0
      })
      .collect();
    let activated = Rc::new(Cell::new(None));

    tree::attach::<Linux>(root, list, 0);

    for (index, row) in rows.iter().enumerate() {
      let activated = Rc::clone(&activated);

      tree::handle_activate::<Linux>(*row, move || activated.set(Some(index)));
      tree::attach::<Linux>(list, *row, index);
    }

    let mut shell = Shell::new("test", 100, 100);

    shell.set_root(root_node);
    shell.frame();

    for expected in [0, 1, 2, 0] {
      shell.key(&key("Tab", true));

      let focus = shell.focus().expect("focus moved").id.get();

      assert_eq!(focus, rows[expected]);
      assert!(Shell::holds(
        shell.focus().expect("focused"),
        State::FocusVisible
      ));
    }

    shell.key(&key("Enter", true));
    assert_eq!(activated.get(), Some(0));
  });
}
