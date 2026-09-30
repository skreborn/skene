//! Suspense and error boundaries: a swap preserves the content's state, the losing
//! branch is destroyed with the owner, and nesting composes.

use std::{cell::Cell, fmt, rc::Rc};

use skene_core::{Boundary, Errors, Flexibility, HasChild, HasChildren, StaticValue, View};
use skene_headless::{Headless, box_props, children, end_turn, is_live, outline, reset};
use skene_reactive::{Get, Owner, Set, Signal};

type Column = skene_common::Column<Headless>;
type Container = skene_common::Container<Headless>;
type Flexible<C = skene_common::NoChild> = skene_common::Flexible<Headless, C>;
type Text = skene_common::Text<Headless>;
type Suspense<C = skene_common::NoChild> = skene_common::Suspense<Headless, C>;
type ErrorBoundary<C = skene_common::NoChild> = skene_common::ErrorBoundary<Headless, C>;

#[derive(Debug)]
struct Failed;

impl fmt::Display for Failed {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("failed")
  }
}

impl std::error::Error for Failed {}

/// A wrapper around a boundary annotates its content and its fallback alike.
#[test]
fn a_wrapper_annotates_a_boundarys_content_and_fallback() {
  reset();

  let owner = Owner::new();
  let boundary = Rc::new(Cell::new(None::<Boundary>));
  let errors = Rc::new(Cell::new(None::<Errors>));
  let root = owner.enter(|| {
    let boundary = Rc::clone(&boundary);
    let errors = Rc::clone(&errors);

    Column::new()
      .set_children((
        Flexible::new().set_grow_factor(2.).set_child(
          Suspense::new()
            .set_fallback(Container::new())
            .set_child_builder(move || {
              boundary.set(Boundary::current());

              Container::new()
            }),
        ),
        Flexible::new().set_grow_factor(3.).set_child(
          ErrorBoundary::new()
            .set_fallback_builder(|_: Errors| Container::new())
            .set_child_builder(move || {
              errors.set(Errors::current());

              Container::new()
            }),
        ),
      ))
      .build()
  });
  let grown = || {
    children(root)
      .into_iter()
      .filter_map(|node| box_props(node)?.style.flex)
      .collect::<Vec<_>>()
  };
  let grow = |factor| StaticValue::from(Flexibility::grow(factor));

  assert_eq!(grown(), [grow(2.), grow(3.)]);

  boundary.get().expect("the suspense's content").enter();
  errors
    .get()
    .expect("the error boundary's content")
    .report(Failed);
  end_turn();

  assert_eq!(grown(), [grow(2.), grow(3.)]);

  owner.dispose();
}

/// Pending work swaps the fallback in and the content out without destroying either:
/// the content's signal and node survive, and both branches are destroyed when the
/// owner is disposed.
#[test]
fn suspense_swap_preserves_the_content_and_disposal_destroys_the_fallback() {
  reset();

  let owner = Owner::new();
  let boundary = Rc::new(Cell::new(None::<Boundary>));
  let local = Rc::new(Cell::new(None::<Signal<i32>>));
  let root = owner.enter(|| {
    let boundary = Rc::clone(&boundary);
    let local = Rc::clone(&local);

    Container::new()
      .set_child(
        Suspense::new()
          .set_fallback(Text::new("loading"))
          .set_child_builder(move || {
            boundary.set(Boundary::current());
            local.set(Some(Signal::new(1)));

            Text::new("content")
          }),
      )
      .build()
  });
  let boundary = boundary.get().expect("the content built under a boundary");
  let local = local.get().expect("the content built");

  assert_eq!(outline(root), "Box\n  Run \"content\"\n");

  let content = children(root)
    .into_iter()
    .find(|node| skene_headless::text(*node).as_deref() == Some("content"))
    .unwrap();

  boundary.enter();

  assert_eq!(outline(root), "Box\n  Run \"loading\"\n");
  assert!(is_live(content));

  let fallback = children(root)
    .into_iter()
    .find(|node| skene_headless::text(*node).as_deref() == Some("loading"))
    .unwrap();

  local.set(2);
  boundary.leave();

  assert_eq!(outline(root), "Box\n  Run \"content\"\n");
  assert!(children(root).contains(&content));
  assert!(is_live(fallback));
  assert_eq!(local.try_get(), Some(2));

  owner.dispose();

  assert!(!is_live(fallback));
  assert!(!is_live(content));
  assert_eq!(local.try_get(), None);
}

/// A nested suspense counts as one pending task toward the enclosing boundary while
/// its own work is pending.
#[test]
fn nested_suspense_pends_the_enclosing_boundary_once() {
  reset();

  let owner = Owner::new();
  let outer = Rc::new(Cell::new(None::<Boundary>));
  let inner = Rc::new(Cell::new(None::<Boundary>));
  let root = owner.enter(|| {
    let outer = Rc::clone(&outer);
    let inner = Rc::clone(&inner);

    Container::new()
      .set_child(
        Suspense::new()
          .set_fallback(Text::new("outer loading"))
          .set_child_builder(move || {
            outer.set(Boundary::current());

            Suspense::new()
              .set_fallback(Text::new("inner loading"))
              .set_child_builder(move || {
                inner.set(Boundary::current());

                Text::new("content")
              })
          }),
      )
      .build()
  });
  let outer = outer.get().unwrap();
  let inner = inner.get().unwrap();

  assert!(outer.is_settled() && inner.is_settled());

  inner.enter();
  inner.enter();
  end_turn();

  assert!(!outer.is_settled());
  assert_eq!(outline(root), "Box\n  Run \"outer loading\"\n");

  inner.leave();
  end_turn();

  assert!(!outer.is_settled());

  inner.leave();
  end_turn();

  assert!(outer.is_settled());
  assert_eq!(outline(root), "Box\n  Run \"content\"\n");

  owner.dispose();
}

/// A reported error swaps the fallback in, built from the boundary's handle; clearing
/// restores the same content node; disposal destroys the content.
#[test]
fn error_boundary_shows_the_fallback_and_restores_the_content() {
  reset();

  let owner = Owner::new();
  let errors = Rc::new(Cell::new(None::<Errors>));
  let root = owner.enter(|| {
    let errors = Rc::clone(&errors);

    Container::new()
      .set_child(
        ErrorBoundary::new()
          .set_fallback_builder(|errors: Errors| {
            Text::derive(move || errors.with(|errors| format!("{} failed", errors.len())))
          })
          .set_child_builder(move || {
            errors.set(Errors::current());

            Text::new("content")
          }),
      )
      .build()
  });
  let errors = errors.get().expect("the content built under a boundary");
  let content = children(root)
    .into_iter()
    .find(|node| skene_headless::text(*node).as_deref() == Some("content"))
    .unwrap();

  assert_eq!(outline(root), "Box\n  Run \"content\"\n");

  errors.report(Failed);

  assert_eq!(outline(root), "Box\n  Run \"1 failed\"\n");
  assert!(is_live(content));

  errors.report(Failed);

  assert_eq!(outline(root), "Box\n  Run \"2 failed\"\n");

  errors.clear();

  assert_eq!(outline(root), "Box\n  Run \"content\"\n");
  assert!(children(root).contains(&content));

  errors.report(Failed);
  owner.dispose();

  assert!(!is_live(content));
}
