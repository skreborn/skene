//! Async in cells: a derived subtree, a suspense boundary over a resource, a
//! scope-bound task, and an error boundary.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::{cell::Cell, rc::Rc};

use skene_tui::{self as tui, prelude::*};

use crate::common::with_driver;

/// A derived subtree swaps its content when its dependency changes.
#[test]
fn derived_swaps_content() {
  with_driver(|| {
    let on = Signal::new(false);

    let _ = tui::mount(|| {
      derived(move || {
        if on.get() {
          Text::new("on").into_any().build()
        }
        else {
          Text::new("off").into_any().build()
        }
      })
    });

    assert_eq!(tui::render_to_text(6, 1), "off\n");

    on.set(true);

    assert_eq!(tui::render_to_text(6, 1), "on\n");
  });
}

/// A suspense shows its fallback until the resource inside settles, then swaps to the
/// content.
#[test]
fn suspense_swaps_fallback_for_loaded_content() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Suspense::new()
        .set_fallback(Text::new("LOADING"))
        .set_child_builder(|| {
          let value = Resource::new(|| async { 42u32 });

          AnyView::new(move || {
            derived(move || match value.get() {
              None => Text::new("hold").into_any().build(),
              Some(value) => Text::new(format!("value {value}")).into_any().build(),
            })
          })
        })
        .into_any()
        .build()
    });

    let loading = tui::render_to_text(12, 2);

    assert!(
      loading.contains("LOADING") && !loading.contains("value"),
      "{loading}"
    );

    assert!(tui::pump_tasks());

    let loaded = tui::render_to_text(12, 2);

    assert!(
      loaded.contains("value 42") && !loaded.contains("LOADING"),
      "{loaded}"
    );
  });
}

/// A spawned task is scope-bound: content torn down before its task lands never runs
/// the continuation.
#[test]
fn tasks_end_with_their_scope() {
  let landed = Rc::new(Cell::new(false));
  let seen = Rc::clone(&landed);

  with_driver(move || {
    let show = Signal::new(true);

    let _ = tui::mount(|| {
      derived(move || {
        if show.get() {
          let landed = Rc::clone(&landed);

          spawn(async move { landed.set(true) });

          Text::new("on").into_any().build()
        }
        else {
          Text::new("off").into_any().build()
        }
      })
    });

    show.set(false);

    let _ = tui::pump_tasks();
  });

  assert!(!seen.get());
}

/// An error boundary swaps to its fallback when a captured handle reports, and clearing
/// restores the content as it was.
#[test]
fn error_boundary_presents_and_clears_reports() {
  with_driver(|| {
    let handle = Rc::new(Cell::new(None::<Errors>));
    let grabbed = Rc::clone(&handle);

    let _ = tui::mount(|| {
      ErrorBoundary::new()
        .set_fallback_builder(|errors| {
          Text::new(errors.with(|errors| format!("failed: {}", errors[0])))
        })
        .set_child_builder(move || {
          grabbed.set(Errors::current());

          Text::new("CONTENT")
        })
        .into_any()
        .build()
    });

    assert!(tui::render_to_text(30, 2).contains("CONTENT"));

    let errors = handle.get().expect("a boundary handle");

    errors.report(std::io::Error::other("boom"));

    let failed = tui::render_to_text(30, 2);

    assert!(
      failed.contains("failed: boom") && !failed.contains("CONTENT"),
      "{failed}"
    );

    errors.clear();

    let restored = tui::render_to_text(30, 2);

    assert!(
      restored.contains("CONTENT") && !restored.contains("failed"),
      "{restored}"
    );
  });
}
