//! The state snapshot in cells: scroll offsets riding it into a fresh driver, and the
//! dev loop's rebuild notice.

#![cfg(all(not(target_arch = "wasm32"), feature = "snapshot"))]

mod common;

use skene_tui::{self as tui, prelude::*};

use crate::common::with_driver;

/// Scroll offsets ride the state snapshot: restored into a fresh driver, the same nodes
/// resume at their prior offsets.
#[test]
fn scroll_offsets_survive_a_snapshot_roundtrip() {
  fn build() {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(4)
        .set_overflow(Overflow::Auto)
        .set_child(
          Column::new().set_children(
            (0..12)
              .map(|index| Text::new(format!("r{index}")))
              .collect::<Vec<_>>(),
          ),
        )
        .into_any()
        .build()
    });
  }

  let bytes = with_driver(|| {
    build();

    let _ = tui::render_to_text(6, 4);

    tui::trigger_mouse_scroll(2, 2, 0, 3);

    assert!(tui::render_to_text(6, 4).contains("r3"));

    tui::reactive::snapshot()
  });
  let restored = with_driver(|| {
    tui::reactive::restore(&bytes);
    build();
    tui::adopt_scroll();

    tui::render_to_text(6, 4)
  });

  assert!(
    restored.contains("r3") && !restored.contains("r0\n"),
    "the restored run resumed at the same offset: {restored}"
  );
}

/// A restored scroll offset reaches the scroll handlers: a lazy column resumes with its
/// scrolled-to items built, not a blank window.
#[test]
fn restored_scroll_rebuilds_a_lazy_column_window() {
  fn build() {
    let _ = tui::mount(|| {
      Container::new()
        .set_height(4)
        .set_child(
          LazyColumn::new()
            .set_item_height(1.)
            .set_overscan(2)
            .derive_items(
              || 0..1000,
              |index| *index,
              |index| Text::new(format!("row {index}")),
            ),
        )
        .into_any()
        .build()
    });
  }

  let bytes = with_driver(|| {
    build();

    let _ = tui::render_to_text(12, 6);

    tui::trigger_mouse_scroll(2, 1, 0, 500);

    assert!(tui::render_to_text(12, 6).contains("row 500"));

    tui::reactive::snapshot()
  });
  let restored = with_driver(|| {
    tui::reactive::restore(&bytes);
    build();
    tui::adopt_scroll();

    let _ = tui::render_to_text(12, 6);

    tui::render_to_text(12, 6)
  });

  assert!(
    restored.contains("row 500") && !restored.contains("row 0\n"),
    "the resumed window builds without a manual scroll: {restored}"
  );
}

/// The dev rebuild notice presents as a modal over the app and dismisses with `Escape`.
#[test]
fn dev_notice_modal_presents_and_dismisses() {
  with_driver(|| {
    let notice = Signal::new(None::<String>);

    let _ = tui::mount(|| Text::new("app content").into_any().build());
    let _ = tui::mount(|| tui::dev_notice_view(notice).into_view().build());

    notice.set(Some("expected `;`".to_owned()));

    let open = tui::render_to_text(40, 10);

    assert!(
      open.contains("Rebuild failed") && open.contains("expected `;`"),
      "the notice presented: {open}"
    );

    tui::trigger_key("Escape");

    let dismissed = tui::render_to_text(40, 10);

    assert!(
      !dismissed.contains("Rebuild failed") && dismissed.contains("app content"),
      "Escape dismissed the notice: {dismissed}"
    );
  });
}
