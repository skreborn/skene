//! Layout and paint in cells: the layout views, the wrappers, sizing, wrapping, and a
//! third-party layout view on the built-ins' footing.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{AnyKind, Children, FlexChild, Seal};
use skene_terminal::{self as terminal, prelude::*, testing};

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
        .set_spans([
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
          GridTrack::Length(4),
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
    let adaptive = GridTrack::Adaptive(TrackLimit::Length(4), TrackLimit::Flex(1.));
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
        .set_columns(vec![GridTrack::Length(4), GridTrack::Flex(1.)])
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
            .add_track(GridTrack::Length(2))
            .add_line(CONTENT)
            .add_track(GridTrack::Length(3))
            .add_track(GridTrack::Length(3))
            .add_line(END)
            .add_track(GridTrack::Length(2)),
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
      .set_spacing(Spacing::from(1).when(Media::MinWidth(10), Spacing::from(3)))
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

    fn set_spacing(mut self, spacing: u32) -> Self {
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
          RichText::new().set_spans([
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
          RichText::new().set_spans([
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
        .set_child(RichText::new().set_spans([
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
        .set_spans([
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
        .set_spans([
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
        .set_spans([
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
