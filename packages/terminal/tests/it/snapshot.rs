//! The state snapshot in cells: scroll offsets and focus riding it into a fresh platform,
//! and the dev loop's rebuild notice.

use skene_simple_terminal::prelude::*;
use skene_terminal::{self as terminal, HasChild, prelude::*, testing};

use crate::common::with_platform;

/// Scroll offsets ride the state snapshot: restored into a fresh platform, the same nodes
/// resume at their prior offsets.
#[test]
fn scroll_offsets_survive_a_snapshot_roundtrip() {
  fn build() {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(4)
        .set_overflow(Overflow::ScrollIfNeeded)
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

  let bytes = with_platform(|| {
    terminal::reactive::record_snapshots();
    build();

    let _ = testing::render_to_text(6, 4);

    testing::trigger_mouse_scroll(2, 2, 0, 3);

    assert!(testing::render_to_text(6, 4).contains("r3"));

    terminal::reactive::snapshot()
  });
  let restored = with_platform(|| {
    terminal::reactive::restore(&bytes);
    build();
    terminal::adopt_restoration();

    testing::render_to_text(6, 4)
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
    let _ = terminal::mount(|| {
      Container::new()
        .set_height(4)
        .set_child(
          LazyColumn::new()
            .set_item_height(1)
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

  let bytes = with_platform(|| {
    terminal::reactive::record_snapshots();
    build();

    let _ = testing::render_to_text(12, 6);

    testing::trigger_mouse_scroll(2, 1, 0, 500);

    assert!(testing::render_to_text(12, 6).contains("row 500"));

    terminal::reactive::snapshot()
  });
  let restored = with_platform(|| {
    terminal::reactive::restore(&bytes);
    build();
    terminal::adopt_restoration();

    let _ = testing::render_to_text(12, 6);

    testing::render_to_text(12, 6)
  });

  assert!(
    restored.contains("row 500") && !restored.contains("row 0\n"),
    "the resumed window builds without a manual scroll: {restored}"
  );
}

/// The dev rebuild notice presents as a modal over the app and dismisses with `Escape`.
#[test]
fn dev_notice_modal_presents_and_dismisses() {
  with_platform(|| {
    let notice = Signal::new(None::<String>);

    let _ = terminal::mount(|| Text::new("app content").build());
    let _ = terminal::mount(|| terminal::dev_notice_view(notice).build());

    notice.set(Some("expected `;`".to_owned()));

    let open = testing::render_to_text(40, 10);

    assert!(
      open.contains("Rebuild failed") && open.contains("expected `;`"),
      "the notice presented: {open}"
    );

    testing::trigger_key("Escape");

    let dismissed = testing::render_to_text(40, 10);

    assert!(
      !dismissed.contains("Rebuild failed") && dismissed.contains("app content"),
      "Escape dismissed the notice: {dismissed}"
    );
  });
}

/// Focus rides the state snapshot: restored into a fresh platform, the same control holds
/// focus without a keystroke, so the next `Space` activates it.
#[test]
fn focus_survives_a_snapshot_roundtrip() {
  use std::{cell::Cell, rc::Rc};

  fn build(hits: &Rc<Cell<u32>>) {
    let hits = Rc::clone(hits);

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          SimpleButton::new("first"),
          SimpleButton::new("second").handle_activate(move || hits.set(hits.get() + 1)),
        ))
        .into_any()
        .build()
    });
  }

  let hits = Rc::new(Cell::new(0));
  let bytes = with_platform(|| {
    terminal::reactive::record_snapshots();
    build(&hits);

    let _ = testing::render_to_text(12, 2);

    testing::trigger_key("Tab");
    testing::trigger_key("Tab");

    terminal::reactive::snapshot()
  });

  with_platform(|| {
    terminal::reactive::restore(&bytes);
    build(&hits);
    terminal::adopt_restoration();

    let _ = testing::render_to_text(12, 2);

    testing::trigger_key(" ");

    assert_eq!(hits.get(), 1, "the second button resumed focused");
  });
}

/// Focus follows a keyed item's key: the list reordered on the next run, the same item
/// resumes focused, so the next `Space` activates it and not the one at its old place.
#[test]
fn focus_follows_a_keyed_item_across_a_reorder() {
  use std::{cell::RefCell, rc::Rc};

  fn build(items: Vec<u8>, hits: &Rc<RefCell<Vec<u8>>>) {
    let hits = Rc::clone(hits);

    let _ = terminal::mount(move || {
      Column::new()
        .derive_children(
          move || items.clone(),
          |item| *item,
          move |item| {
            let hits = Rc::clone(&hits);

            SimpleButton::new(format!("b{item}"))
              .handle_activate(move || hits.borrow_mut().push(item))
          },
        )
        .into_any()
        .build()
    });
  }

  let hits = Rc::new(RefCell::new(Vec::new()));
  let bytes = with_platform(|| {
    terminal::reactive::record_snapshots();
    build(vec![1, 2, 3], &hits);

    let _ = testing::render_to_text(12, 3);

    testing::trigger_key("Tab");
    testing::trigger_key("Tab");

    terminal::reactive::snapshot()
  });

  with_platform(|| {
    terminal::reactive::restore(&bytes);
    build(vec![3, 1, 2], &hits);
    terminal::adopt_restoration();

    let _ = testing::render_to_text(12, 3);

    testing::trigger_key(" ");

    assert_eq!(
      hits.borrow().as_slice(),
      [2],
      "the same item resumed focused"
    );
  });
}

/// A text entry's caret and selection ride the snapshot with its focus: the same field
/// resumes with the caret where it was, so typing continues there.
#[test]
fn a_caret_survives_a_snapshot_roundtrip() {
  fn build(value: Signal<String>) {
    let _ = terminal::mount(move || SimpleTextField::new().bind_value(value).build());
  }

  let bytes = with_platform(|| {
    terminal::reactive::record_snapshots();
    let value = Signal::new(String::from("ac"));

    build(value);

    let _ = testing::render_to_text(12, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_key("ArrowLeft");

    terminal::reactive::snapshot()
  });

  with_platform(|| {
    let value = Signal::new(String::from("ac"));

    terminal::reactive::restore(&bytes);
    build(value);
    terminal::adopt_restoration();

    let _ = testing::render_to_text(12, 1);

    testing::trigger_key("b");

    assert_eq!(
      value.get(),
      "abc",
      "the caret resumed before the last character"
    );
  });

  let bytes = with_platform(|| {
    terminal::reactive::record_snapshots();
    let value = Signal::new(String::from("alpha beta"));

    build(value);

    let _ = testing::render_to_text(12, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("End");
    testing::trigger_key("Ctrl+Shift+ArrowLeft");

    terminal::reactive::snapshot()
  });

  with_platform(|| {
    let value = Signal::new(String::from("alpha beta"));

    terminal::reactive::restore(&bytes);
    build(value);
    terminal::adopt_restoration();

    let _ = testing::render_to_text(12, 1);

    testing::trigger_key("x");

    assert_eq!(
      value.get(),
      "alpha x",
      "the selection resumed over the word"
    );
  });
}
