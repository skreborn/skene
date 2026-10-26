//! Async in cells: a derived subtree, a suspense boundary over a resource, a swap that
//! keeps both branches until their owner goes, nested suspense, a scope-bound task, an
//! error boundary, and a wrapper annotating a boundary's either branch.

use std::{cell::Cell, fmt, rc::Rc};

use skene_foundation::Boundary;
use skene_terminal::{platform::terminal::mount, prelude::*, testing};

use crate::common::{blue, colors_at, green, is_live, red, rgb, with_platform};

/// The error the boundary tests report.
#[derive(Debug)]
struct Failed;

impl fmt::Display for Failed {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("failed")
  }
}

impl std::error::Error for Failed {}

/// A derived subtree swaps its content when its dependency changes.
#[test]
fn derived_swaps_content() {
  with_platform(|| {
    let on = Signal::new(false);

    let _ = mount(|| {
      derived(move || {
        if on.get() {
          Text::new("on").build()
        }
        else {
          Text::new("off").build()
        }
      })
    });

    assert_eq!(testing::render_to_text(6, 1), "off\n");

    on.set(true);

    assert_eq!(testing::render_to_text(6, 1), "on\n");
  });
}

/// A suspense shows its fallback until the resource inside settles, then swaps to the
/// content.
#[test]
fn suspense_swaps_fallback_for_loaded_content() {
  with_platform(|| {
    let _ = mount(|| {
      Suspense::new()
        .set_fallback_builder(|| Text::new("LOADING"))
        .set_child_builder(|| {
          let value = Resource::new(|| async { 42u32 });

          AnyView::new(move || {
            derived(move || match value.get() {
              None => Text::new("hold").build(),
              Some(value) => Text::new(format!("value {value}")).build(),
            })
          })
        })
        .into_any()
        .build()
    });

    let loading = testing::render_to_text(12, 2);

    assert!(
      loading.contains("LOADING") && !loading.contains("value"),
      "{loading}"
    );

    assert!(testing::pump_tasks());

    let loaded = testing::render_to_text(12, 2);

    assert!(
      loaded.contains("value 42") && !loaded.contains("LOADING"),
      "{loaded}"
    );
  });
}

/// Pending work swaps the fallback in and the content out without destroying either:
/// the content's signal and node survive the swap, and both branches go with their
/// owner.
#[test]
fn suspense_swap_preserves_the_content_and_disposal_destroys_the_fallback() {
  with_platform(|| {
    let shown = Signal::new(true);
    let boundary = Rc::new(Cell::new(None::<Boundary>));
    let local = Rc::new(Cell::new(None::<Signal<i32>>));

    let _ = mount({
      let boundary = Rc::clone(&boundary);
      let local = Rc::clone(&local);

      move || {
        derived(move || {
          let boundary = Rc::clone(&boundary);
          let local = Rc::clone(&local);

          if !shown.get() {
            return Text::new("gone").build();
          }

          Suspense::new()
            .set_fallback_builder(|| Text::new("loading"))
            .set_child_builder(move || {
              boundary.set(Boundary::current());
              local.set(Some(Signal::new(1)));

              Text::new("content")
            })
            .into_any()
            .build()
        })
      }
    });
    let boundary = boundary.get().expect("the content built under a boundary");
    let local = local.get().expect("the content built");

    assert_eq!(testing::render_to_text(10, 1), "content\n");

    let content = testing::find_by_text("content").expect("the content");

    boundary.enter();

    assert_eq!(testing::render_to_text(10, 1), "loading\n");
    assert_eq!(testing::find_by_text("content"), None);
    assert!(is_live(content), "the content waits aside");

    let fallback = testing::find_by_text("loading").expect("the fallback");

    local.set(2);
    boundary.leave();

    assert_eq!(testing::render_to_text(10, 1), "content\n");
    assert_eq!(testing::find_by_text("content"), Some(content));
    assert!(is_live(fallback), "the fallback waits aside");
    assert_eq!(local.try_get(), Some(2));

    shown.set(false);

    assert_eq!(testing::render_to_text(10, 1), "gone\n");
    assert!(!is_live(fallback));
    assert!(!is_live(content));
    assert_eq!(local.try_get(), None);
  });
}

/// A nested suspense counts as one pending task toward the enclosing boundary while
/// its own work is pending.
#[test]
fn nested_suspense_pends_the_enclosing_boundary_once() {
  with_platform(|| {
    let outer = Rc::new(Cell::new(None::<Boundary>));
    let inner = Rc::new(Cell::new(None::<Boundary>));

    let _ = mount({
      let outer = Rc::clone(&outer);
      let inner = Rc::clone(&inner);

      move || {
        Suspense::new()
          .set_fallback_builder(|| Text::new("outer loading"))
          .set_child_builder(move || {
            outer.set(Boundary::current());

            Suspense::new()
              .set_fallback_builder(|| Text::new("inner loading"))
              .set_child_builder(move || {
                inner.set(Boundary::current());

                Text::new("content")
              })
          })
          .into_any()
          .build()
      }
    });
    let outer = outer.get().expect("the outer boundary");
    let inner = inner.get().expect("the inner boundary");

    assert!(outer.is_settled() && inner.is_settled());
    assert_eq!(testing::render_to_text(14, 1), "content\n");

    inner.enter();
    inner.enter();

    assert_eq!(testing::render_to_text(14, 1), "outer loading\n");
    assert!(!outer.is_settled());

    inner.leave();

    assert_eq!(testing::render_to_text(14, 1), "outer loading\n");
    assert!(!outer.is_settled());

    inner.leave();

    assert_eq!(testing::render_to_text(14, 1), "content\n");
    assert!(outer.is_settled());
  });
}

/// A spawned task is scope-bound: content torn down before its task lands never runs
/// the continuation.
#[test]
fn tasks_end_with_their_scope() {
  let landed = Rc::new(Cell::new(false));
  let seen = Rc::clone(&landed);

  with_platform(move || {
    let show = Signal::new(true);

    let _ = mount(|| {
      derived(move || {
        if show.get() {
          let landed = Rc::clone(&landed);

          spawn(async move { landed.set(true) });

          Text::new("on").build()
        }
        else {
          Text::new("off").build()
        }
      })
    });

    show.set(false);

    let _ = testing::pump_tasks();
  });

  assert!(!seen.get());
}

/// An error boundary swaps to its fallback when a captured handle reports, the fallback
/// following each further report; clearing restores the same content node, and the
/// content goes with the boundary's owner.
#[test]
fn error_boundary_presents_and_clears_reports() {
  with_platform(|| {
    let shown = Signal::new(true);
    let handle = Rc::new(Cell::new(None::<Errors>));
    let grabbed = Rc::clone(&handle);

    let _ = mount(move || {
      derived(move || {
        let grabbed = Rc::clone(&grabbed);

        if !shown.get() {
          return Text::new("gone").build();
        }

        ErrorBoundary::new()
          .set_fallback_builder(|errors: Errors| {
            Text::derive(move || {
              errors.with(|errors| format!("{} failed: {}", errors.len(), errors[0]))
            })
          })
          .set_child_builder(move || {
            grabbed.set(Errors::current());

            Text::new("CONTENT")
          })
          .into_any()
          .build()
      })
    });

    assert!(testing::render_to_text(30, 2).contains("CONTENT"));

    let content = testing::find_by_text("CONTENT").expect("the content");
    let errors = handle.get().expect("a boundary handle");

    errors.report(std::io::Error::other("boom"));

    let failed = testing::render_to_text(30, 2);

    assert!(
      failed.contains("1 failed: boom") && !failed.contains("CONTENT"),
      "{failed}"
    );
    assert!(is_live(content), "the content waits aside");

    errors.report(Failed);

    let again = testing::render_to_text(30, 2);

    assert!(again.contains("2 failed: boom"), "{again}");

    errors.clear();

    let restored = testing::render_to_text(30, 2);

    assert!(
      restored.contains("CONTENT") && !restored.contains("failed"),
      "{restored}"
    );
    assert_eq!(testing::find_by_text("CONTENT"), Some(content));

    errors.report(Failed);
    shown.set(false);

    assert!(testing::render_to_text(30, 2).contains("gone"));
    assert!(!is_live(content));
  });
}

/// A wrapper around a boundary annotates its content and its fallback alike: each
/// branch takes the share of the row its wrapper grows it to.
#[test]
fn a_wrapper_annotates_a_boundarys_content_and_fallback() {
  with_platform(|| {
    let boundary = Rc::new(Cell::new(None::<Boundary>));
    let errors = Rc::new(Cell::new(None::<Errors>));

    let _ = mount({
      let boundary = Rc::clone(&boundary);
      let errors = Rc::clone(&errors);

      move || {
        Row::new()
          .set_children((
            Flexible::new()
              .set_flexibility(Flexibility::grow(2.))
              .set_child(
                Suspense::new()
                  .set_fallback_builder(|| Container::new().set_height(1).set_background(green()))
                  .set_child_builder(move || {
                    boundary.set(Boundary::current());

                    Container::new().set_height(1).set_background(red())
                  }),
              ),
            Flexible::new()
              .set_flexibility(Flexibility::grow(3.))
              .set_child(
                ErrorBoundary::new()
                  .set_fallback_builder(|_: Errors| {
                    Container::new().set_height(1).set_background(green())
                  })
                  .set_child_builder(move || {
                    errors.set(Errors::current());

                    Container::new().set_height(1).set_background(blue())
                  }),
              ),
          ))
          .into_any()
          .build()
      }
    });
    let backgrounds = || {
      (0..10)
        .map(|x| colors_at(10, 1, x, 0).1)
        .collect::<Vec<_>>()
    };
    let shares = |first: Color, second: Color| {
      [[Some(rgb(first)); 4].as_slice(), &[Some(rgb(second)); 6]].concat()
    };

    assert_eq!(backgrounds(), shares(red(), blue()));

    boundary.get().expect("the suspense's content").enter();
    errors
      .get()
      .expect("the error boundary's content")
      .report(Failed);

    assert_eq!(
      backgrounds(),
      shares(green(), green()),
      "the fallbacks take the shares the content took"
    );
  });
}
