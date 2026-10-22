//! Text sizing in cells: a run's font size is a scale the terminal draws through
//! OSC 66 once it proves the protocol, and one cell until then—in layout, paint,
//! wrapping, a paragraph's baseline and the selection.

use skene_terminal::{self as terminal, CellScale, prelude::*, testing};

use crate::{common, common::with_platform};

/// The scale at the cell (`x`, `y`) of a `width` x `height` frame.
fn scale_at(width: u16, height: u16, x: usize, y: usize) -> CellScale {
  testing::render_to_scales(width, height)[y * usize::from(width) + x]
}

/// A sized run measures one cell tall until the terminal proves text sizing; then a
/// heading at size two takes two rows and two columns per cell, the run below it
/// moves down, a wrap counts the scaled columns, and a size below one keeps its cells.
#[test]
fn a_size_is_honored_once_the_terminal_proves_it() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Container::new()
            .set_width(10)
            .set_child(Text::new("Big text").set_font_size(2.)),
          Text::new("small").set_font_size(0.5),
          Text::new("after"),
        ))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(12, 6);

    assert!(frame.starts_with("Big text\nsmall\nafter\n"), "{frame}");
    assert_eq!(scale_at(12, 6, 0, 0), CellScale::ONE);

    testing::trigger_text_sizing(true);

    let frame = testing::render_to_text(12, 6);

    assert!(
      frame.starts_with("B i g\n\nt e x t\n\nsmall\nafter\n"),
      "at scale two `Big text` wraps at ten cells and each line is two rows: {frame}"
    );
    assert_eq!(scale_at(12, 6, 0, 0), CellScale::Whole(2));
    assert_eq!(
      scale_at(12, 6, 1, 0),
      CellScale::ONE,
      "the block's other cells"
    );
    assert_eq!(scale_at(12, 6, 2, 0), CellScale::Whole(2), "the next glyph");
    assert_eq!(scale_at(12, 6, 0, 4), CellScale::Fraction(1, 2));
    assert_eq!(
      testing::render_to_scales(12, 6)
        .iter()
        .filter(|scale| **scale == CellScale::Whole(2))
        .count(),
      7,
      "one sized cell per cluster, the space included"
    );

    testing::trigger_text_sizing(false);

    let frame = testing::render_to_text(12, 6);

    assert!(frame.starts_with("Big text\nsmall\nafter\n"), "{frame}");
  });
}

/// A paragraph's line is as tall as its tallest span, and the smaller spans sit on
/// its bottom row as on a baseline; the typography's size sizes every run beneath it.
#[test]
fn a_paragraph_sets_its_smaller_spans_on_the_baseline() {
  with_platform(|| {
    testing::trigger_text_sizing(true);

    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          RichText::new().add_spans([
            Span::new("A").set_font_size(3.),
            Span::new("b").set_font_size(2.),
            Span::new("c"),
          ]),
          Theme::new(Typography::default().set_size(2.)).set_child_builder(|| Text::new("themed")),
        ))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(20, 6);
    let (a_x, a_y) = common::locate(&frame, "A");
    let (b_x, b_y) = common::locate(&frame, "b");
    let (c_x, c_y) = common::locate(&frame, "c");
    let (_, themed_y) = common::locate(&frame, "t h e m e d");

    assert_eq!((a_x, a_y), (0, 0), "{frame}");
    assert_eq!((b_x, b_y), (3, 1), "{frame}");
    assert_eq!((c_x, c_y), (5, 2), "{frame}");
    assert_eq!(themed_y, 3, "the paragraph is three rows tall: {frame}");
    assert_eq!(scale_at(20, 6, 0, 3), CellScale::Whole(2));
  });
}

/// A drag over a heading selects by its blocks: a press in a block's lower row and a
/// release in another's later column take the clusters whose blocks hold them, and
/// the whole block paints selected.
#[test]
fn the_selection_takes_the_clusters_whose_blocks_are_hit() {
  with_platform(|| {
    testing::trigger_text_sizing(true);

    let _ = terminal::mount(|| {
      Text::new("abc")
        .set_font_size(2.)
        .set_selectable(true)
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(12, 4);

    testing::trigger_mouse_press(2, 1);
    testing::trigger_mouse_drag(5, 0);
    testing::trigger_mouse_release(5, 0);

    let reverse = testing::render_to_reverse(12, 4);

    assert!(!reverse[0] && reverse[2] && reverse[3] && reverse[12 + 2] && reverse[12 + 5]);
    assert!(!reverse[6], "{reverse:?}");

    testing::trigger_mouse_right_click(3, 1);

    let frame = testing::render_to_text(12, 4);
    let (x, y) = common::locate(&frame, "Copy");

    testing::trigger_mouse_click(x, y);

    assert_eq!(testing::take_copied().as_deref(), Some("bc"));
  });
}

/// An `em` is the typography's size in cells once the terminal proves text sizing, and
/// one cell until then, as every size is.
#[test]
fn an_em_is_the_typography_size_in_cells() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Row::new().set_children((
            Container::new().set_width(Expr::font_size(3.)),
            Text::new("x"),
          )),
          Theme::new(Typography::default().set_size(2.)).set_child_builder(|| {
            Row::new().set_children((
              Container::new().set_width(Expr::font_size(3.)),
              Text::new("y"),
            ))
          }),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 2), "   x\n   y\n");

    testing::trigger_text_sizing(true);

    let frame = testing::render_to_text(12, 3);

    assert!(frame.starts_with("   x\n      y"), "{frame}");
  });
}

/// A group's size reaches the spans beneath it through the typography it redefines,
/// where a span sets none of its own.
#[test]
fn a_groups_size_reaches_its_spans() {
  with_platform(|| {
    testing::trigger_text_sizing(true);

    let _ = terminal::mount(|| {
      RichText::new()
        .add_spans([Span::group([Span::new("b")]).set_font_size(2.)])
        .into_any()
        .build()
    });

    let _ = testing::render_to_text(4, 2);

    assert_eq!(scale_at(4, 2, 0, 0), CellScale::Whole(2));
  });
}

/// A tab in a sized run is one blank cell at the run's scale wherever it falls, the
/// layout placing what follows where the paint draws it.
#[test]
fn a_tab_in_a_sized_run_is_one_sized_blank() {
  with_platform(|| {
    testing::trigger_text_sizing(true);

    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          RichText::new().add_spans([Span::new("a"), Span::new("\tb").set_font_size(2.)]),
          Text::new("|"),
        ))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(12, 3);

    assert!(frame.starts_with("   b |\na\n"), "{frame}");
    assert_eq!(scale_at(12, 3, 1, 0), CellScale::Whole(2));
    assert_eq!(scale_at(12, 3, 3, 0), CellScale::Whole(2));
  });
}
