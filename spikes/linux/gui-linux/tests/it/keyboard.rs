//! The shell's keyboard and pointer rules beyond the controls' own: an acting row answers
//! keys as a button does, link runs are tab stops `Enter` follows, focus scrolls into view and
//! the keys scroll.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{
  Dimension, State,
  seam::{
    content::{Run, contents},
    node::{LayoutKind, NodeKind, NodeSpec},
    record::{Record, keys},
    report::Modifiers,
    tree,
  },
};
use skene_gui_linux::{Linux, keys::KeyInput, node::Node, shell::Shell};

use super::{boxed, node, owned};

fn key(name: &str, down: bool, modifiers: Modifiers) -> KeyInput {
  KeyInput {
    event: skene_foundation::seam::report::KeyEvent {
      key: name.to_owned(),
      code: String::new(),
      down,
      repeat: false,
      modifiers,
    },
    text: None,
    keycode: 0,
    composing: None,
    composed: false,
  }
}

fn tap(shell: &mut Shell, name: &str) {
  shell.key(&key(name, true, Modifiers::default()));
  shell.key(&key(name, false, Modifiers::default()));
  shell.frame();
}

/// An acting row of height 40 whose activations count into the cell returned.
fn row(record: Record) -> (skene_foundation::NodeId, Node, Rc<Cell<u32>>) {
  let (row, row_node) = node(
    NodeSpec::new(NodeKind::Layout(LayoutKind::Box)).acting(),
    record,
  );
  let count = Rc::new(Cell::new(0));

  {
    let count = Rc::clone(&count);

    tree::handle_activate::<Linux>(row, move || count.set(count.get() + 1));
  }

  (row, row_node, count)
}

/// A text of one link run, `Open the docs`, leading to `destination`.
fn link(destination: &'static str) -> (skene_foundation::NodeId, Node) {
  let (text, text_node) = node(NodeSpec::new(NodeKind::Text), Record::new());

  tree::bind::<Linux, _>(text, contents::RUNS, move || {
    Rc::from(vec![Run {
      text: Rc::from("Open the docs"),
      props: Rc::from(vec![]),
      destination: Some(Rc::from(destination)),
    }])
  });

  (text, text_node)
}

#[test]
fn a_row_answers_keys_as_a_button_does() {
  owned(|| {
    let (root, root_node) = boxed(Record::new());
    let mut record = Record::new();

    record.set(keys::HEIGHT, Dimension::Length(40.));

    let (row, _, count) = row(record);

    tree::attach::<Linux>(root, row, 0);

    let mut shell = Shell::new("test", 200, 100);

    shell.set_root(root_node);
    shell.frame();
    shell.key(&key("Tab", true, Modifiers::default()));
    assert_eq!(shell.focus().map(|focus| focus.id.get()), Some(row));

    let control = Modifiers {
      control: true,
      ..Modifiers::default()
    };

    shell.key(&key("Enter", true, control));
    assert_eq!(count.get(), 0, "Control-Enter is not the row's");

    shell.key(&key(" ", false, Modifiers::default()));
    assert_eq!(
      count.get(),
      0,
      "a Space release with no press activates nothing"
    );

    shell.key(&key(" ", true, Modifiers::default()));
    shell.frame();
    assert!(
      Shell::holds(shell.focus().expect("focused"), State::Pressed),
      "Space shows the row pressed"
    );

    shell.key(&key("Escape", true, Modifiers::default()));
    shell.key(&key(" ", false, Modifiers::default()));
    assert_eq!(count.get(), 0, "Escape cancels a held Space");
    assert_eq!(
      shell.focus().map(|focus| focus.id.get()),
      Some(row),
      "the Escape that cancels keeps focus"
    );

    tap(&mut shell, " ");
    assert_eq!(count.get(), 1, "Space activates on its release");

    shell.key(&key("Enter", true, Modifiers::default()));
    assert_eq!(count.get(), 2, "Enter activates on its press");
  });
}

#[test]
fn tab_reaches_link_runs_and_enter_follows_them() {
  owned(|| {
    let (root, root_node) = node(
      NodeSpec::new(NodeKind::Layout(LayoutKind::Column)),
      Record::new(),
    );
    let mut record = Record::new();

    record.set(keys::HEIGHT, Dimension::Length(30.));

    let (first, ..) = row(record);
    let (text, _) = link("https://example.com/a");
    let followed = Rc::new(std::cell::RefCell::new(Vec::new()));

    {
      let followed = Rc::clone(&followed);

      tree::handle_follow::<Linux, ()>(root, move |destination| {
        followed.borrow_mut().push(destination.to_string());
      });
    }

    tree::attach::<Linux>(root, first, 0);
    tree::attach::<Linux>(root, text, 1);

    let mut shell = Shell::new("test", 300, 100);

    shell.set_root(root_node);
    shell.configure(300, 100);
    shell.frame();

    let mut stops = Vec::new();

    for _ in 0..3 {
      shell.key(&key("Tab", true, Modifiers::default()));
      stops.push((
        shell.focus().map(|focus| focus.id.get()),
        shell.focused_link(),
      ));
    }

    assert_eq!(
      stops,
      [
        (Some(first), None),
        (Some(text), Some(0)),
        (Some(first), None)
      ],
      "the link run is a tab stop in the scene's order"
    );

    shell.key(&key("Tab", true, Modifiers::default()));
    tap(&mut shell, " ");
    assert!(followed.borrow().is_empty(), "Space does not follow a link");

    shell.key(&key("Enter", true, Modifiers::default()));
    assert_eq!(*followed.borrow(), ["https://example.com/a"]);
  });
}

#[test]
fn focus_scrolls_into_view_and_the_keys_scroll() {
  owned(|| {
    let mut scroll_record = Record::new();

    scroll_record.set(keys::HEIGHT, Dimension::Length(100.));

    let (scroll, scroll_node) = node(
      NodeSpec::new(NodeKind::Layout(LayoutKind::Scroll {
        horizontal: false,
        vertical: true,
      })),
      scroll_record,
    );
    let (list, _) = node(
      NodeSpec::new(NodeKind::Layout(LayoutKind::Column)),
      Record::new(),
    );

    for index in 0..10 {
      let mut record = Record::new();

      record.set(keys::HEIGHT, Dimension::Length(40.));

      let (row, ..) = row(record);

      tree::attach::<Linux>(list, row, index);
    }

    tree::attach::<Linux>(scroll, list, 0);

    let mut shell = Shell::new("test", 200, 100);

    shell.set_root(Rc::clone(&scroll_node));
    shell.configure(200, 100);
    shell.frame();

    for _ in 0..6 {
      shell.key(&key("Tab", true, Modifiers::default()));
      shell.frame();
    }

    let view = scroll_node.window_rect();
    let focused = shell.focus().expect("focused").window_rect();

    assert!(
      focused.y >= view.y && focused.y + focused.height <= view.y + view.height,
      "the sixth row {focused:?} shows within {view:?}"
    );
    assert_eq!(
      scroll_node.scroll.get(),
      (0., 140.),
      "scrolled as little as shows it"
    );

    shell.key(&key("Escape", true, Modifiers::default()));
    shell.key(&key("Home", true, Modifiers::default()));
    shell.frame();
    assert_eq!(
      scroll_node.scroll.get(),
      (0., 0.),
      "Home scrolls to the start"
    );

    shell.key(&key("PageDown", true, Modifiers::default()));
    shell.frame();
    assert_eq!(
      scroll_node.scroll.get(),
      (0., 87.5),
      "PageDown scrolls most of the box"
    );

    shell.key(&key("ArrowDown", true, Modifiers::default()));
    shell.frame();
    assert_eq!(
      scroll_node.scroll.get(),
      (0., 127.5),
      "an arrow scrolls a line"
    );

    shell.key(&key("End", true, Modifiers::default()));
    shell.frame();
    assert_eq!(
      scroll_node.scroll.get(),
      (0., 300.),
      "End scrolls to the end"
    );
  });
}
