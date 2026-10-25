//! Selection in cells: drags anchored in source space, the word and run gestures, the
//! copy menu, and the selection colors a typography defines.

use skene_terminal::{self as terminal, Label, behavior::Activate, prelude::*, testing};

use crate::common::{green, red, rgb, white, with_platform};

/// The selection highlight the tests read back.
fn highlight() -> Color {
  Color::srgb(0.2, 0.35, 0.53)
}

/// A typography painting selections in white on the highlight.
fn typography() -> Typography {
  Typography::default()
    .set_selection_ink(white())
    .set_selection_surface(highlight())
}

/// Mounts `content` under the test typography.
fn mount_themed(content: impl FnOnce() -> AnyView + 'static) {
  let _ = terminal::mount(|| {
    Theme::new(typography())
      .set_child_builder(content)
      .into_any()
      .build()
  });
}

/// A drag over a wrapped run highlights the selected cells, and `Copy` yields the text
/// as it appears in the source: the soft wrap's collapsed space restored, no line
/// break inserted.
#[test]
fn drag_selects_and_copies_the_source_flow() {
  with_platform(|| {
    mount_themed(|| {
      Container::new()
        .set_width(8)
        .set_child(Text::new("alpha beta gamma").set_selectable(true))
        .into_any()
    });
    let _ = testing::render_to_text(20, 6);

    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_drag(2, 1);
    testing::trigger_mouse_release(2, 1);

    let colors = testing::render_to_colors(20, 6);

    assert_eq!(colors[1], (Some(rgb(white())), Some(rgb(highlight()))));
    assert_eq!(colors[0].1, None);
    assert_eq!(
      colors[20 + 1].1,
      Some(rgb(highlight())),
      "the wrapped line continues"
    );
    assert_eq!(colors[20 + 2].1, None, "the focus is exclusive");

    testing::trigger_mouse_right_click(2, 1);

    let menu = testing::render_to_text(20, 6);

    assert!(menu.contains("Copy"), "the menu opened: {menu}");

    testing::trigger_mouse_click(4, 2);

    assert_eq!(testing::take_copied().as_deref(), Some("lpha be"));
  });
}

/// A drag over right-to-left text selects between the cells' left edges as drawn, the
/// first character rightmost: it highlights the cells it crossed and copies them in the
/// order the source holds them.
#[test]
fn a_drag_over_right_to_left_text_selects_what_it_crosses() {
  with_platform(|| {
    mount_themed(|| {
      Text::new("ab \u{5d0}\u{5d1}\u{5d2} cd")
        .set_selectable(true)
        .into_any()
    });

    assert_eq!(
      testing::render_to_text(12, 3),
      "ab \u{5d2}\u{5d1}\u{5d0} cd\n\n\n"
    );

    testing::trigger_mouse_press(5, 0);
    testing::trigger_mouse_drag(3, 0);
    testing::trigger_mouse_release(3, 0);

    let colors = testing::render_to_colors(12, 3);
    let highlighted: Vec<usize> = (0..12)
      .filter(|column| colors[*column].1 == Some(rgb(highlight())))
      .collect();

    assert_eq!(highlighted, [3, 4], "the cells of \u{5d2} and \u{5d1}");

    testing::trigger_mouse_right_click(4, 0);

    let menu = testing::render_to_text(12, 3);

    assert!(menu.contains("Copy"), "the menu opened: {menu}");

    testing::trigger_mouse_click(6, 1);

    assert_eq!(testing::take_copied().as_deref(), Some("\u{5d1}\u{5d2}"));
  });
}

/// A double click selects the word under the pointer; a triple click the whole run.
#[test]
fn double_and_triple_click_select_word_and_run() {
  let select = |clicks: usize| {
    with_platform(|| {
      mount_themed(|| Text::new("alpha beta").set_selectable(true).into_any());
      let _ = testing::render_to_text(20, 4);

      for _ in 0..clicks {
        testing::trigger_mouse_click(7, 0);
      }

      testing::trigger_mouse_right_click(0, 0);

      let _ = testing::render_to_text(20, 4);

      testing::trigger_mouse_click(2, 1);

      testing::take_copied()
    })
  };

  assert_eq!(select(2).as_deref(), Some("beta"));
  assert_eq!(select(3).as_deref(), Some("alpha beta"));
}

/// A drag spans runs in document order—joined by newlines on copy—and dragging past
/// every run clamps to the nearest position.
#[test]
fn selection_spans_runs_in_document_order() {
  with_platform(|| {
    mount_themed(|| {
      Column::new()
        .set_children((
          Text::new("first").set_selectable(true),
          Text::new("second").set_selectable(true),
        ))
        .into_any()
    });
    let _ = testing::render_to_text(40, 8);

    testing::trigger_mouse_press(2, 0);
    testing::trigger_mouse_drag(30, 3);
    testing::trigger_mouse_release(30, 3);
    testing::trigger_mouse_right_click(3, 3);

    let _ = testing::render_to_text(40, 8);

    testing::trigger_mouse_click(5, 4);

    assert_eq!(testing::take_copied().as_deref(), Some("rst\nsecond"));
  });
}

/// A button's label never selects—an explicit opt-in notwithstanding—and a plain run
/// stays unselectable by default; neither press opens the copy menu.
#[test]
fn control_chrome_and_opted_out_runs_do_not_select() {
  with_platform(|| {
    mount_themed(|| {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .set_label(Label::new(Text::new("Press me").set_selectable(true)))
            .into_any(),
          Text::new("plain").into_any(),
        ))
        .into_any()
    });
    let _ = testing::render_to_text(20, 4);

    testing::trigger_mouse_press(1, 0);
    testing::trigger_mouse_drag(6, 0);
    testing::trigger_mouse_release(6, 0);
    testing::trigger_mouse_right_click(1, 0);

    let button_menu = testing::render_to_text(20, 4);

    assert!(
      !button_menu.contains("Copy"),
      "no menu over a label: {button_menu}"
    );

    testing::trigger_mouse_press(1, 1);
    testing::trigger_mouse_drag(4, 1);
    testing::trigger_mouse_release(4, 1);
    testing::trigger_mouse_right_click(1, 1);

    let plain_menu = testing::render_to_text(20, 4);

    assert!(
      !plain_menu.contains("Copy"),
      "no menu over an opt-out: {plain_menu}"
    );
    assert_eq!(testing::take_copied(), None);
  });
}

/// A typography's selection colors inherit down the subtree, and a run's own setter
/// overrides the theme.
#[test]
fn typography_sets_selection_colors() {
  with_platform(|| {
    mount_themed(|| {
      Column::new()
        .set_children((
          Text::new("themed").set_selectable(true),
          Text::new("custom")
            .set_selectable(true)
            .set_selection_background(green()),
        ))
        .into_any()
    });
    let _ = testing::render_to_text(20, 4);

    for _ in 0..3 {
      testing::trigger_mouse_click(0, 0);
    }

    assert_eq!(
      testing::render_to_colors(20, 4)[0],
      (Some(rgb(white())), Some(rgb(highlight())))
    );

    for _ in 0..3 {
      testing::trigger_mouse_click(0, 1);
    }

    assert_eq!(
      testing::render_to_colors(20, 4)[20],
      (Some(rgb(white())), Some(rgb(green())))
    );
  });
}

/// `Escape` peels the transient layers in order: first the copy menu, then the
/// selection.
#[test]
fn escape_closes_the_menu_then_clears_the_selection() {
  with_platform(|| {
    mount_themed(|| Text::new("alpha beta").set_selectable(true).into_any());
    let _ = testing::render_to_text(20, 4);

    testing::trigger_mouse_click(2, 0);
    testing::trigger_mouse_click(2, 0);
    testing::trigger_mouse_right_click(0, 0);

    assert!(testing::render_to_text(20, 4).contains("Copy"));

    testing::trigger_key("Escape");

    let kept = testing::render_to_colors(20, 4);

    assert!(!testing::render_to_text(20, 4).contains("Copy"));
    assert_eq!(
      kept[2].1,
      Some(rgb(highlight())),
      "the selection survives the menu"
    );

    testing::trigger_key("Escape");

    assert_eq!(
      testing::render_to_colors(20, 4)[2].1,
      None,
      "the second Escape clears it"
    );
  });
}

/// Rich text flows its spans as one wrapping paragraph—each span keeping its own ink
/// and the linked span the scheme's link blue—and an opted-in paragraph selects and
/// copies across span boundaries as one source flow.
#[test]
fn rich_text_flows_wraps_and_selects_as_one_paragraph() {
  with_platform(|| {
    mount_themed(|| {
      Container::new()
        .set_width(12)
        .set_child(RichText::new().add_spans([
          Span::new("plain "),
          Span::new("red").set_color(red()),
          Span::new(" and "),
          Span::new("docs").set_link("https://example.com"),
        ]))
        .into_any()
    });

    let frame = testing::render_to_text(14, 4);
    let colors = testing::render_to_colors(14, 4);

    assert_eq!(frame, "plain red\nand docs\n\n\n");
    assert_eq!(colors[6].0, Some(rgb(red())), "the red span keeps its ink");
    assert_eq!(colors[0].0, None, "the plain span stays default");
    assert_eq!(
      colors[14 + 4].0,
      Some((102, 153, 255)),
      "the linked span inks in link blue"
    );
  });

  with_platform(|| {
    mount_themed(|| {
      Container::new()
        .set_width(12)
        .set_child(RichText::new().add_spans([
          Span::new("plain ").set_selectable(true),
          Span::new("red").set_color(red()),
          Span::new(" and "),
          Span::new("docs").set_link("https://example.com"),
        ]))
        .into_any()
    });
    let _ = testing::render_to_text(14, 4);

    testing::trigger_mouse_press(0, 0);
    testing::trigger_mouse_drag(8, 1);
    testing::trigger_mouse_release(8, 1);
    testing::trigger_mouse_right_click(0, 1);

    let _ = testing::render_to_text(14, 4);

    testing::trigger_mouse_click(2, 2);

    assert_eq!(
      testing::take_copied().as_deref(),
      Some("plain red and docs")
    );
  });
}
