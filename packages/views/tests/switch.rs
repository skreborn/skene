//! The switch: a branch is built once, keeps its state while another is shown, and is
//! destroyed with the switch's owner.

mod common;

use std::{cell::Cell, rc::Rc};

use skene_core::{Flexibility, HasChildren, StaticValue, View};
use skene_headless::{Headless, box_props, children, is_live, outline, reset};
use skene_reactive::{Get, Owner, Set, Signal};

type Container = skene_views::Container<Headless>;
type Column = skene_views::Column<Headless>;
type Text = skene_views::Text<Headless>;
type Flexible<C = skene_views::NoChild> = skene_views::Flexible<Headless, C>;
type Switch<K> = skene_views::Switch<Headless, K>;

/// Flipping away and back reattaches the same branch node with its signal intact, the
/// branch builds exactly once, and an unknown key shows nothing.
#[test]
fn switch_keeps_a_branch_and_its_state_across_a_flip() {
  reset();

  let owner = Owner::new();
  let key = Signal::opaque("a");
  let builds = Rc::new(Cell::new(0));
  let local = Rc::new(Cell::new(None::<Signal<i32>>));
  let root = owner.enter(|| {
    let builds = Rc::clone(&builds);
    let local = Rc::clone(&local);

    Column::new()
      .set_child(
        Switch::new()
          .derive_key(move || key.get())
          .set_case("a", move || {
            builds.set(builds.get() + 1);
            local.set(Some(Signal::new(1)));

            Container::new().set_child(Text::new("a"))
          })
          .set_case("b", || Text::new("b")),
      )
      .build()
  });
  let branch = children(root)[2];
  let local = local.get().expect("the first branch built");

  assert_eq!(outline(root), "Box\n  Box\n    Run \"a\"\n");
  assert_eq!(builds.get(), 1);

  local.set(5);
  key.set("b");

  assert_eq!(outline(root), "Box\n  Run \"b\"\n");
  assert!(is_live(branch));
  assert_eq!(local.try_get(), Some(5));

  key.set("missing");

  assert_eq!(outline(root), "Box\n");

  key.set("a");

  assert_eq!(outline(root), "Box\n  Box\n    Run \"a\"\n");
  assert_eq!(children(root)[2], branch);
  assert_eq!(builds.get(), 1);
  assert_eq!(local.try_get(), Some(5));

  key.set("b");
  owner.dispose();

  assert!(!is_live(branch));
  assert_eq!(local.try_get(), None);

  owner.dispose();
}

/// A wrapper around a switch annotates whichever branch is built, including one built
/// after a later flip.
#[test]
fn switch_forwards_the_pending_annotation_to_every_branch() {
  reset();

  let owner = Owner::new();
  let key = Signal::new(1);
  let root = owner.enter(|| {
    Column::new()
      .set_child(
        Flexible::new().set_grow_factor(3.).set_child(
          Switch::new()
            .derive_key(move || key.get())
            .set_case(1, Container::new)
            .set_case(2, Container::new),
        ),
      )
      .build()
  });
  let flex = |node| box_props(node).and_then(|props| props.style.flex);
  let expected = Some(StaticValue::from(Flexibility::grow(3.)));

  assert_eq!(flex(children(root)[2]), expected);

  key.set(2);

  assert_eq!(flex(children(root)[2]), expected);
  assert_eq!(flex(root), None);

  owner.dispose();
}
