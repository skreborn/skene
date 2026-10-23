//! Layout and paint in cells: the layout views, the wrappers, sizing, wrapping, and a
//! third-party layout view on the built-ins' footing.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{AnyKind, Children, FlexChild, Seal};
use skene_terminal::{
  self as terminal,
  Definition,
  Length,
  TextEntry,
  Token,
  TokenName,
  prelude::*,
  testing,
};

use crate::common::{colors_at, locate, red, rgb, white, with_platform};

/// A column stacks its runs on successive rows.
#[test]
fn column_stacks_runs() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((Text::new("alpha"), Text::new("beta")))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 4), "alpha\nbeta\n\n\n");
  });
}

/// A background fill is opaque: whatever painted beneath it clears, rather than
/// showing through as recolored characters.
#[test]
fn background_fills_cover_content_beneath() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(1)
        .set_child(
          Stack::new().set_children((
            Text::new("under"),
            Container::new()
              .set_width(6)
              .set_height(1)
              .set_background(Color::srgb(0.5, 0.5, 0.5)),
          )),
        )
        .into_any()
        .build()
    });

    assert_eq!(
      testing::render_to_text(6, 1),
      "\n",
      "the fill covered the text"
    );
  });
}

/// A pinned bare text run honors its edges inside a stack.
#[test]
fn pinned_text_honors_its_edges() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(12)
        .set_height(3)
        .set_child(
          Stack::new().add_child(
            Pinned::new()
              .set_edges(Inset {
                top: Dimension::Auto,
                end: Dimension::Length(0),
                bottom: Dimension::Length(0),
                start: Dimension::Auto,
              })
              .set_child(Text::new("pin")),
          ),
        )
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 3), "\n\n         pin\n");
  });
}

/// A flexible bare text run takes the row's free space, pushing later children over.
#[test]
fn flexible_text_takes_free_space() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          Text::new("a"),
          Flexible::new()
            .set_flexibility(Flexibility::grow(1.))
            .set_child(Text::new("b")),
          Text::new("c"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 1), "ab         c\n");
  });
}

/// Text wraps at its container's width, breaking at spaces.
#[test]
fn text_wraps_at_the_container_width() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_child(Text::new("alpha beta"))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 4), "alpha\nbeta\n\n\n");
  });
}

/// Right-to-left text paints in the engine's visual order, inside a left-to-right line as
/// a browser's document reads it: its first character rightmost, a paragraph's spans
/// reordered as their text is.
#[test]
fn right_to_left_text_paints_in_visual_order() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Text::new("ab \u{5d0}\u{5d1}\u{5d2} cd"),
          RichText::new().add_spans([
            Span::new("\u{5d0}\u{5d1} "),
            Span::new("\u{5d2}\u{5d3}"),
            Span::new(" ef"),
          ]),
        ))
        .into_any()
        .build()
    });

    assert_eq!(
      testing::render_to_text(12, 2),
      "ab \u{5d2}\u{5d1}\u{5d0} cd\n\u{5d3}\u{5d2} \u{5d1}\u{5d0} ef\n"
    );
  });
}

/// A newline ending a run's text opens a line, and an empty text lays out one.
#[test]
fn a_final_newline_and_an_empty_text_open_a_line() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Text::new("one\n"),
          Text::new(""),
          Text::new("two\n\n"),
          Text::new("three"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 8), "one\n\n\ntwo\n\n\nthree\n\n");
  });
}

/// A border draws box-drawing characters, and the root child fills the viewport.
#[test]
fn border_draws_a_frame_and_the_root_child_fills_the_viewport() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(7)
        .set_border_width(Inset::all(1))
        .set_border_color(white())
        .set_child(Text::new("hi"))
        .into_any()
        .build()
    });

    assert_eq!(
      testing::render_to_text(10, 4),
      "┌─────┐\n│hi   │\n│     │\n└─────┘\n"
    );
  });
}

/// A zero-width border takes no cells and draws nothing.
#[test]
fn a_zero_width_border_takes_no_cells() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(1)
        .set_border_width(Inset::all(0))
        .set_border_color(white())
        .set_child(Text::new("hi"))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(6, 2), "hi\n\n");
  });
}

/// Bold is a plain `bool` on a run, and derived text re-renders on a signal change.
#[test]
fn bold_and_derived_text() {
  with_platform(|| {
    let count = Signal::new(1u32);

    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Text::new("title").set_bold(true),
          Text::derive(move || format!("n={}", count.get())),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 2), "title\nn=1\n");
    assert_eq!(
      testing::render_to_bold(8, 2),
      [[true; 5].as_slice(), &[false; 11]].concat(),
      "the title alone is bold"
    );

    count.set(2);

    assert_eq!(testing::render_to_text(8, 2), "title\nn=2\n");
  });
}

/// A group's weight reaches the spans beneath it through the typography it redefines,
/// and the span beside the group keeps the typography's.
#[test]
fn a_groups_weight_reaches_its_spans() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      RichText::new()
        .add_spans([
          Span::new("a"),
          Span::group([Span::new("bc")]).set_font_weight(700.),
        ])
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(4, 1), "abc\n");
    assert_eq!(testing::render_to_bold(4, 1), [false, true, true, false]);
  });
}

/// Grid tracks size the columns: a fixed track, then the rest split evenly.
#[test]
fn grid_tracks_size_columns() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Grid::new()
        .set_columns(vec![
          GridTrack::length(4),
          GridTrack::Flex(1.),
          GridTrack::Flex(1.),
        ])
        .set_children((Text::new("a"), Text::new("b"), Text::new("c")))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 1), "a   b   c\n");
  });
}

/// An adaptive grid fits as many columns as its minimum allows and shares the rest of
/// the width among them.
#[test]
fn an_adaptive_grid_fits_its_columns() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Grid::adaptive(4)
        .set_children((
          Text::new("a"),
          Text::new("b"),
          Text::new("c"),
          Text::new("d"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(10, 2), "a    b\nc    d\n");
  });
}

/// A track list holding two adaptive tracks is dropped whole, as CSS drops it, leaving
/// the children to implicit rows.
#[test]
fn a_second_adaptive_track_drops_the_list() {
  with_platform(|| {
    let adaptive = GridTrack::adaptive(4, TrackLimit::Flex(1.));
    let _ = terminal::mount(move || {
      Grid::new()
        .set_columns(vec![adaptive, adaptive])
        .set_children((Text::new("a"), Text::new("b")))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(10, 2), "a\nb\n");
  });
}

/// A child placed by an area's name lands on the area's tracks.
#[test]
fn a_named_area_places_its_child() {
  const HEAD: GridName = GridName::new("head");
  const MAIN: GridName = GridName::new("main");

  with_platform(|| {
    let _ = terminal::mount(|| {
      Grid::new()
        .set_columns(vec![GridTrack::length(4), GridTrack::Flex(1.)])
        .set_areas(GridAreas::new(["head head", "nav main"]))
        .set_children((
          GridCell::new()
            .set_row(MAIN)
            .set_column(MAIN)
            .set_child(Text::new("m")),
          GridCell::new()
            .set_row(HEAD)
            .set_column(HEAD)
            .set_child(Text::new("h")),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(10, 2), "h\n    m\n");
  });
}

/// A child placed between named lines spans the tracks between them.
#[test]
fn named_lines_place_a_child() {
  const CONTENT: GridName = GridName::new("content");
  const END: GridName = GridName::new("end");

  with_platform(|| {
    let _ = terminal::mount(|| {
      Grid::new()
        .set_columns(
          GridTemplate::new()
            .add_track(2)
            .add_line(CONTENT)
            .add_track(3)
            .add_track(3)
            .add_line(END)
            .add_track(2),
        )
        .set_children((GridCell::new()
          .set_column(Line {
            start: GridPlacement::Named(CONTENT, 1),
            end: GridPlacement::Named(END, 1),
          })
          .set_child(Text::new("middle")),))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 1), "  middle\n");
  });
}

/// A grid child placement spans tracks.
#[test]
fn grid_placement_spans_tracks() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Grid::uniform(2)
        .set_children((
          GridCell::new()
            .set_column(Line::span(2))
            .set_child(Container::new().set_child(Text::new("wide"))),
          Text::new("l"),
          Text::new("r"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 2), "wide\nl   r\n");
  });
}

/// Flex grow distributes the free space between children.
#[test]
fn flex_grow_distributes_space() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          Flexible::new()
            .set_flexibility(Flexibility::grow(1.))
            .set_child(
              Container::new()
                .set_background(Color::srgb(0.2, 0.2, 0.8))
                .set_child(Text::new("a")),
            ),
          Container::new().set_width(3).set_child(Text::new("b")),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 1), "a        b\n");
  });
}

/// The one route a link in these tests names.
#[derive(Clone, PartialEq)]
struct Home;

impl terminal::router::Route for Home {
  fn parse(path: &str) -> Option<Self> {
    (path == "/").then_some(Self)
  }

  fn to_path(&self) -> String {
    "/".to_owned()
  }
}

/// A wrapper or a view of one child without its child paints nothing, takes no room and
/// shows a reader nothing, its fallback included: the texts around them sit the
/// column's spacing apart and no more, and a reader finds the texts alone.
#[test]
fn a_wrapper_without_its_child_takes_no_room() {
  let childless = with_platform(|| {
    let live = Signal::new(true);

    let _ = terminal::mount(move || {
      Column::new()
        .set_spacing(1)
        .set_children((
          Text::new("a"),
          Flexible::new().set_flexibility(Flexibility::grow(1.)),
          Tooltip::new("about"),
          Visibility::new(),
          Overlay::popup().set_open(true),
          Overlay::modal().set_open(true).set_label("Settings"),
        ))
        .add_child(ContextMenu::new().set_items([MenuEntry::new("Inspect")]))
        .add_child(Keymap::new().handle_shortcut(Shortcut::new(Modifiers::CTRL, "s"), || {}))
        .add_child(Link::new(Home))
        .add_child(Enhance::new(live.read_only()).set_fallback_builder(|| Text::new("fallback")))
        .add_child(Suspense::new().set_fallback_builder(|| Text::new("loading")))
        .add_child(ErrorBoundary::new().set_fallback_builder(|_| Text::new("failed")))
        .add_child(Theme::new(Typography::default()))
        .add_child(Text::new("b"))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(4, 5), "a\n\nb\n\n\n");

    testing::access_outline()
  });
  let texts = with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((Text::new("a"), Text::new("b")))
        .into_any()
        .build()
    });

    testing::access_outline()
  });

  assert_eq!(childless, texts);
}

/// A pinned wrapper around a derived subtree lands its edges on the content box, on
/// every rebuild, and on no other node.
#[test]
fn pinned_annotates_the_content_of_a_derived_subtree() {
  with_platform(|| {
    let version = Signal::new(0);

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Text::new("top"),
          Stack::new().derive_child(move || {
            let version = version.get();

            Pinned::new()
              .set_edges(Inset {
                top: Dimension::Length(1),
                end: Dimension::Auto,
                bottom: Dimension::Auto,
                start: Dimension::Length(2),
              })
              .set_child(
                Container::new()
                  .set_child(Container::new().set_child(Text::new(format!("x{version}")))),
              )
          }),
        ))
        .into_any()
        .build()
    });

    assert_eq!(
      testing::render_to_text(6, 4),
      "top\n\n  x0\n\n",
      "the content box alone takes the edges, in the stack below the text"
    );

    version.set(1);

    assert_eq!(testing::render_to_text(6, 4), "top\n\n  x1\n\n");
  });
}

/// A grid cell around a derived subtree lands its placements and both alignment axes
/// on the content box, on every rebuild.
#[test]
fn grid_cell_annotates_the_content_of_a_derived_subtree() {
  with_platform(|| {
    let version = Signal::new(0);

    let _ = terminal::mount(move || {
      Grid::uniform(2)
        .set_rows(vec![GridTrack::Length(1), GridTrack::Length(3)])
        .add_child(Text::new("t"))
        .derive_child(move || {
          let version = version.get();

          GridCell::new()
            .set_row(Line::at(2))
            .set_column(Line::span(2))
            .set_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::Center))
            .set_child(Container::new().set_child(Text::new(format!("c{version}"))))
        })
        .into_any()
        .build()
    });

    assert_eq!(
      testing::render_to_text(8, 4),
      "t\n\n      c0\n\n",
      "the content sits in the second row across both columns, at its end and centered"
    );

    version.set(1);

    assert_eq!(testing::render_to_text(8, 4), "t\n\n      c1\n\n");
  });
}

/// A wrapper whose child creates no node leaves its properties on nothing: the next box
/// built takes none of them.
#[test]
fn a_wrapper_around_nothing_annotates_no_later_box() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Stack::new()
        .set_children((
          Pinned::new().set_edges(Inset::all(2)).set_child(empty()),
          Container::new().set_child(Text::new("x")),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(4, 3), "x\n\n\n");
  });
}

/// A wrapper without its child annotates no later box: the box after it in a stack, a
/// column and a grid takes none of its properties.
#[test]
fn a_wrapper_without_its_child_annotates_no_later_box() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Stack::new().set_children((
            Pinned::new().set_edges(Inset::all(2)),
            Container::new().set_child(Text::new("s")),
          )),
          Container::new().set_height(3).set_child(
            Column::new().set_children((
              Flexible::new().set_flexibility(Flexibility::grow(1.)),
              Container::new()
                .set_background(red())
                .set_child(Text::new("c")),
            )),
          ),
          Grid::uniform(2).set_children((
            GridCell::new().set_column(Line::at(2)),
            Container::new().set_child(Text::new("g")),
          )),
        ))
        .into_any()
        .build()
    });

    assert_eq!(
      crate::common::rows(&testing::render_to_text(6, 5)),
      "s\nc\n\n\ng\n"
    );

    let red_rows = (1..4)
      .filter(|row| colors_at(6, 5, 0, *row).1 == Some(rgb(red())))
      .count();

    assert_eq!(red_rows, 1, "the box after the flexible did not grow");
  });
}

/// A wrapper given its child takes its setters still.
#[test]
fn a_wrapper_given_its_child_takes_its_setters() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      let wrapper = Pinned::new().set_child(Container::new().set_child(Text::new("x")));

      Stack::new()
        .set_children((wrapper.set_edges(Inset::all(1)),))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(3, 3), "\n x\n\n");
  });
}

/// A negative margin pulls a box back over its sibling, while a negative padding takes
/// no room, as a browser takes none below zero.
#[test]
fn a_negative_margin_overlaps_and_a_negative_padding_takes_no_room() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Text::new("abc"),
          Container::new()
            .set_margin(Inset {
              top: Dimension::Length(-1),
              end: Dimension::Length(0),
              bottom: Dimension::Length(0),
              start: Dimension::Length(1),
            })
            .set_child(Text::new("x")),
          Container::new()
            .set_padding(Inset::all(-2))
            .set_child(Text::new("p")),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(4, 3), "axc\np\n\n");
  });
}

/// A width below zero is dropped as CSS drops it: a box sizes to its content and a field
/// to its value, as where no width is set.
#[test]
fn a_negative_width_is_dropped() {
  with_platform(|| {
    let value = Signal::new("hi".to_owned());
    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Row::new().set_children((
            Container::new().set_width(-4).set_child(Text::new("ab")),
            Text::new("c"),
          )),
          Row::new().set_children((
            BasicTextField::new(TextEntry::new().bind_value(value)).set_width(-4),
            Text::new("|"),
          )),
          Row::new().set_children((
            BasicTextField::new(TextEntry::new().bind_value(value)),
            Text::new("|"),
          )),
        ))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(20, 3);
    let lines: Vec<&str> = frame.lines().collect();

    assert!(lines[0].starts_with("abc"), "{frame}");
    assert_eq!(lines[1].find('|'), lines[2].find('|'), "{frame}");
    assert!(lines[1].find('|') > Some(2), "{frame}");
  });
}

/// A width below zero under a condition that holds lays out as a width below zero
/// does in the base, as the web renders it: the box sizes to its content, and a
/// padding below zero under a condition takes no room.
#[test]
fn a_negative_variant_lays_out_as_a_negative_base() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Row::new().set_children((
            Container::new()
              .set_width(Dimension::from(6).when(Media::min_width(1), -4))
              .set_child(Text::new("ab")),
            Text::new("c"),
          )),
          Row::new().set_children((
            Container::new()
              .set_padding(Inset::<i32>::all(2).when(Media::min_width(1), Inset::new(-1, 0, 1, 0)))
              .set_child(Text::new("p")),
            Text::new("q"),
          )),
        ))
        .into_any()
        .build()
    });

    assert_eq!(
      testing::render_to_text(8, 2),
      "abc
p q
"
    );
  });
}

/// A width expression that evaluates below zero lays out at zero, as CSS clamps a
/// `calc()`, never as a dropped width the box would stretch to.
#[test]
fn a_negative_width_expression_lays_out_at_zero() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((Container::new()
          .set_width(Expr::font_size(1.) - 5)
          .set_height(1)
          .set_background(red()),))
        .into_any()
        .build()
    });

    let painted = testing::render_to_colors(12, 2)[..12]
      .iter()
      .filter(|colors| colors.1 == Some(rgb(red())))
      .count();

    assert_eq!(painted, 0);
  });
}

/// A length written as an `f32` rounds to the nearest cell at the setter: a width of
/// 2.5 takes three cells, a padding of 0.6 one and a spacing of 1.4 one.
#[test]
fn a_fractional_length_rounds_to_the_nearest_cell() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Row::new()
        .set_spacing(1.4)
        .set_children((
          Container::new().set_width(2.5).set_child(Text::new("a")),
          Container::new()
            .set_padding(Inset::horizontal(0.6))
            .set_child(Text::new("b")),
          Text::new("c"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 1), "a    b  c\n");
  });
}

/// A fraction width resolves against the parent's extent.
#[test]
fn fraction_width_resolves_against_parent() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Row::new()
        .set_children((
          Container::new()
            .set_width(Dimension::Fraction(0.5))
            .set_child(Text::new("half")),
          Text::new("x"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 1), "half  x\n");
  });
}

/// A sized container fills its child to the content box; an unsized one hugs it.
#[test]
fn a_container_fills_when_sized_and_hugs_when_not() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(9)
        .set_height(3)
        .set_border_width(Inset::all(1))
        .set_border_color(white())
        .set_child(Container::new().set_background(red()))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(9, 3, 1, 1).1, Some(rgb(red())));
    assert_eq!(colors_at(9, 3, 7, 1).1, Some(rgb(red())));
  });

  with_platform(|| {
    let _ = terminal::mount(|| {
      Row::new()
        .set_item_alignment(ItemAlignment::Start)
        .add_child(
          Container::new()
            .set_border_width(Inset::all(1))
            .set_border_color(white())
            .set_child(Text::new("hi")),
        )
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(9, 4), "┌──┐\n│hi│\n└──┘\n\n");
  });
}

/// A container places a child that fills neither axis by its alignment.
#[test]
fn a_container_aligns_its_child() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::End))
        .set_child(Text::new("ab"))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(6, 3), "\n\n    ab\n");
  });
}

/// A layout property varies by condition on the value, so the terminal and the web
/// express responsiveness the same declarative way.
#[test]
fn layout_properties_vary_by_condition() {
  fn row() -> impl View {
    Row::new()
      .set_spacing(Spacing::from(1).when(Media::min_width(10), Spacing::from(3)))
      .set_children((Text::new("ab"), Text::new("cd")))
  }

  with_platform(|| {
    let _ = terminal::mount(|| row().build());

    assert_eq!(testing::render_to_text(12, 2), "ab   cd\n\n");
  });

  with_platform(|| {
    let _ = terminal::mount(|| row().build());

    assert_eq!(testing::render_to_text(8, 2), "ab cd\n\n");
  });
}

const GUTTER: TokenRef<Length> = TokenRef::new("gutter");

/// A palette holding a gutter length.
#[derive(Debug, Clone, PartialEq)]
struct Gutters {
  /// The room between a row's children.
  gutter: Token<Length>,
}

impl<V: From<Definition<Length>> + 'static> TokenSet<V> for Gutters {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.gutter.define()]
  }
}

/// A row's layout properties reach its layout: a spacing that is a token resolves
/// through the theme to the palette's length, an expression over it doubles it in a
/// reversed row, and an arrangement and an item alignment place the children.
#[test]
fn layout_properties_reach_the_layout_and_a_token_spacing_resolves() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Theme::new(Gutters {
        gutter: Token::new(GUTTER.name().as_str(), 2.),
      })
      .set_child_builder(|| {
        Column::new().set_children((
          Row::new()
            .set_spacing(GUTTER)
            .set_children((Text::new("a"), Text::new("b")))
            .into_any(),
          Row::reversed()
            .set_spacing(Expr::<Length>::from(GUTTER) * 2.)
            .set_children((Text::new("a"), Text::new("b")))
            .into_any(),
          Container::new()
            .set_width(8)
            .set_height(3)
            .set_child(
              Row::new()
                .set_arrangement(Arrangement::SpaceBetween)
                .set_item_alignment(ItemAlignment::Center)
                .set_children((Text::new("a"), Text::new("b"))),
            )
            .into_any(),
        ))
      })
      .into_any()
      .build()
    });

    assert_eq!(
      testing::render_to_text(10, 5),
      "a  b\n    b    a\n\na      b\n\n"
    );
  });
}

/// A media condition on a layout's padding switches with the terminal's size, whether
/// spelled on the inset, on a plain spacing, or on a plain number.
#[test]
fn a_media_condition_on_padding_switches_with_the_terminals_size() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_spacing(Spacing::from(1).when(Media::min_width(10), 2))
        .set_padding(Inset::all(0).when(Media::min_width(10), Inset::all(1)))
        .set_children((
          Text::new("a").into_any(),
          Container::new()
            .set_width(2)
            .set_aspect_ratio(1f32.when(Media::min_width(10), 2.))
            .set_background(red())
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let boxed = |width: u16| {
      let colors = testing::render_to_colors(width, 6);

      (0..6)
        .filter(|row| colors[row * usize::from(width)].1 == Some(rgb(red())))
        .collect::<Vec<_>>()
    };

    assert_eq!(testing::render_to_text(12, 6), "\n a\n\n\n\n\n");
    assert!(boxed(12).is_empty(), "the box starts a column in");
    assert_eq!(colors_at(12, 6, 1, 4).1, Some(rgb(red())));
    assert_eq!(
      colors_at(12, 6, 1, 5).1,
      None,
      "twice as wide as it is tall"
    );
    assert_eq!(testing::render_to_text(8, 6), "a\n\n\n\n\n\n");
    assert_eq!(
      boxed(8),
      [2, 3],
      "as tall as it is wide, a row below the text"
    );
  });
}

/// A uniform grid lays out one equal track per column, and its two-axis alignment
/// places each item on both axes while its row arrangement places the row.
#[test]
fn a_uniform_grid_lays_out_equal_tracks_and_splits_its_alignment() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(12)
        .set_height(3)
        .set_child(
          Grid::uniform(3)
            .set_item_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::Start))
            .set_row_arrangement(Arrangement::Center)
            .set_children((Text::new("a"), Text::new("b"), Text::new("c"))),
        )
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 3), "\n   a   b   c\n\n");
  });
}

/// A stack's derived alignment places its child on both axes and follows the signal.
#[test]
fn a_stacks_derived_alignment_drives_both_axes() {
  with_platform(|| {
    let alignment = Signal::new(Alignment::CENTER);
    let _ = terminal::mount(move || {
      Container::new()
        .set_width(5)
        .set_height(3)
        .set_child(
          Stack::new()
            .derive_item_alignment(move || alignment.get())
            .add_child(Text::new("x")),
        )
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(5, 3), "\n  x\n\n");

    alignment.set(Alignment::new(ItemAlignment::Start, ItemAlignment::End));

    assert_eq!(testing::render_to_text(5, 3), "\n\nx\n");
  });
}

/// The equal-footing gate: a layout view written outside the facade, against its
/// public API alone, matches the built-in it parallels—typed children (a [`Flexible`]
/// wrapper included) and its own layout knob—rendering cell-for-cell identically.
#[test]
fn a_third_party_layout_view_matches_the_built_in() {
  #[must_use]
  struct Toolbar {
    base: LayoutBox<FlexChild>,
  }

  impl Toolbar {
    fn new() -> Self {
      Self {
        base: LayoutBox::new(Layout::Flex {
          axis: Axis::Horizontal,
          reversed: false,
        }),
      }
    }

    fn set_spacing(mut self, spacing: i32) -> Self {
      self.base = self.base.set_spacing(Spacing {
        horizontal: spacing,
        vertical: 0,
      });

      self
    }
  }

  impl HasChildren<terminal::Terminal> for Toolbar {
    type Kind = FlexChild;

    fn children_mut(&mut self, seal: Seal) -> &mut Children<terminal::Terminal, FlexChild> {
      self.base.children_mut(seal)
    }
  }

  impl AnyKind for Toolbar {}

  impl skene_foundation::View<terminal::Terminal> for Toolbar {
    fn build(self) -> terminal::NodeId {
      skene_foundation::View::build(self.base)
    }
  }

  let third_party = with_platform(|| {
    let _ = terminal::mount(|| {
      Toolbar::new()
        .set_spacing(1)
        .set_children((
          Text::new("ab"),
          Flexible::new()
            .set_flexibility(Flexibility::grow(1.))
            .set_child(Text::new("cd")),
          Text::new("ef"),
        ))
        .into_any()
        .build()
    });

    testing::render_to_text(14, 3)
  });
  let built_in = with_platform(|| {
    let _ = terminal::mount(|| {
      Row::new()
        .set_spacing(1)
        .set_children((
          Text::new("ab"),
          Flexible::new()
            .set_flexibility(Flexibility::grow(1.))
            .set_child(Text::new("cd")),
          Text::new("ef"),
        ))
        .into_any()
        .build()
    });

    testing::render_to_text(14, 3)
  });

  assert_eq!(third_party, built_in);
  assert!(third_party.contains("ab"), "the frame painted content");
}

/// A view inside a paragraph flows as an inline-level box: measured by its own layout,
/// wrapping with the text around it, painted where its cells landed, and hit there.
#[test]
fn an_inline_box_flows_with_the_paragraph() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = terminal::mount(|| {
      Container::new()
        .set_width(10)
        .set_child(
          RichText::new().add_spans([
            Span::new("see "),
            Span::view(
              Container::new()
                .set_padding(Inset::horizontal(1))
                .set_background(red())
                .handle_click(move || counted.set(counted.get() + 1))
                .set_child(Text::new("new")),
            ),
            Span::new(" here"),
          ]),
        )
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(12, 3);

    assert_eq!(frame, "see  new\nhere\n\n");

    let (column, row) = locate(&frame, "new");

    assert_eq!(column, 5);
    assert_eq!(colors_at(12, 3, 4, 0).1, Some(rgb(red())));
    assert_eq!(colors_at(12, 3, 8, 0).1, Some(rgb(red())));
    assert_eq!(colors_at(12, 3, 3, 0).1, None);
    assert_eq!(colors_at(12, 3, 9, 0).1, None);

    testing::trigger_mouse_click(column, row);

    assert_eq!(clicks.get(), 1);
  });
}

/// An inline box as tall as two rows raises its line to two rows, the text beside it on
/// the line's bottom row as on a baseline, and the next line flows below it.
#[test]
fn a_tall_inline_box_raises_its_line() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(12)
        .set_child(
          RichText::new().add_spans([
            Span::new("see "),
            Span::view(
              Container::new()
                .set_height(2)
                .set_background(red())
                .set_child(Text::new("new")),
            ),
            Span::new(" here and more"),
          ]),
        )
        .into_any()
        .build()
    });

    assert_eq!(
      testing::render_to_text(12, 4),
      "    new\nsee     here\nand more\n\n"
    );
    assert_eq!(colors_at(12, 4, 4, 1).1, Some(rgb(red())));
  });
}

/// An inline box wider than what is left of a line moves to the next one whole.
#[test]
fn an_inline_box_never_splits_across_lines() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_child(RichText::new().add_spans([
          Span::new("ab "),
          Span::view(Container::new().set_child(Text::new("wide"))),
        ]))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 3), "ab\nwide\n\n");
  });
}

/// A span under the pointer is hovered, taking its hovered style, and a click on it
/// runs its handler, and none on the span beside it.
#[test]
fn a_span_is_hovered_and_clicked() {
  with_platform(|| {
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = terminal::mount(move || {
      RichText::new()
        .add_spans([
          Span::new("plain "),
          Span::new("click")
            .set_color(red().when(State::Hovered, white()))
            .handle_click(move || counted.set(counted.get() + 1)),
        ])
        .into_any()
        .build()
    });

    assert_eq!(colors_at(12, 1, 6, 0).0, Some(rgb(red())));

    testing::trigger_mouse_move(7, 0);

    assert_eq!(colors_at(12, 1, 6, 0).0, Some(rgb(white())));

    testing::trigger_mouse_click(7, 0);
    testing::trigger_mouse_click(1, 0);

    assert_eq!(clicks.get(), 1);
  });
}

/// Copying a selection across an inline box takes the text the box holds, never its
/// placeholder.
#[test]
fn copying_across_an_inline_box_takes_its_text() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      RichText::new()
        .add_spans([
          Span::new("see ").set_selectable(true),
          Span::view(Text::new("new")),
          Span::new(" here"),
        ])
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(14, 1);

    testing::trigger_mouse_press(0, 0);
    testing::trigger_mouse_drag(12, 0);
    testing::trigger_mouse_release(12, 0);
    testing::trigger_key("Ctrl+c");

    assert_eq!(testing::take_copied().as_deref(), Some("see new here"));
  });
}

/// An inline box inside a group span sits where its placeholder flowed, so a press on
/// it reports a position within the box.
#[test]
fn a_grouped_inline_box_places_its_content() {
  with_platform(|| {
    let pressed = Rc::new(Cell::new((f32::NAN, f32::NAN)));
    let recorded = Rc::clone(&pressed);

    let _ = terminal::mount(move || {
      RichText::new()
        .add_spans([
          Span::new("ab "),
          Span::group([Span::view(
            Container::new()
              .handle_pointer_down(move |event| recorded.set((event.local_x, event.local_y)))
              .set_child(Text::new("box")),
          )]),
        ])
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 1), "ab box\n");

    testing::trigger_mouse_press(4, 0);

    assert_eq!(pressed.get(), (1.5, 0.5));
  });
}
