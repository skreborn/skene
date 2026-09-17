//! Layout and paint in cells: the layout views, the wrappers, sizing, wrapping, and a
//! third-party layout view on the built-ins' footing.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::{cell::Cell, rc::Rc};

use skene_tui::{
  self as tui,
  core::{AnyKind, Children, FlexChild, Seal},
  prelude::*,
};

use crate::common::{colors_at, locate, red, rgb, white, with_platform};

/// A column stacks its runs on successive rows.
#[test]
fn column_stacks_runs() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Column::new()
        .set_children((Text::new("alpha"), Text::new("beta")))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(12, 4), "alpha\nbeta\n\n\n");
  });
}

/// A background fill is opaque: whatever painted beneath it clears, rather than
/// showing through as recolored characters.
#[test]
fn background_fills_cover_content_beneath() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(1)
        .set_child(
          Stack::new().set_children((
            Text::new("under"),
            Container::new()
              .set_width(6)
              .set_height(1)
              .set_color(Color::srgb(0.5, 0.5, 0.5)),
          )),
        )
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(6, 1), "\n", "the fill covered the text");
  });
}

/// A pinned bare text run honors its edges inside a stack.
#[test]
fn pinned_text_honors_its_edges() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(12)
        .set_height(3)
        .set_child(
          Stack::new().set_child(
            Pinned::new()
              .set_edges(Inset {
                top: Dimension::Auto,
                right: Dimension::Length(0),
                bottom: Dimension::Length(0),
                left: Dimension::Auto,
              })
              .set_child(Text::new("pin")),
          ),
        )
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(12, 3), "\n\n         pin\n");
  });
}

/// A flexible bare text run takes the row's free space, pushing later children over.
#[test]
fn flexible_text_takes_free_space() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Row::new()
        .set_children((
          Text::new("a"),
          Flexible::new()
            .set_grow_factor(1.)
            .set_child(Text::new("b")),
          Text::new("c"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(12, 1), "ab         c\n");
  });
}

/// Text wraps at its container's width, breaking at spaces.
#[test]
fn text_wraps_at_the_container_width() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_child(Text::new("alpha beta"))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(12, 4), "alpha\nbeta\n\n\n");
  });
}

/// A border draws box-drawing characters, and the root child fills the viewport.
#[test]
fn border_draws_a_frame_and_the_root_child_fills_the_viewport() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(7)
        .set_border(Border::new(1, white()))
        .set_child(Text::new("hi"))
        .into_any()
        .build()
    });

    assert_eq!(
      tui::render_to_text(10, 4),
      "┌─────┐\n│hi   │\n│     │\n└─────┘\n"
    );
  });
}

/// A zero-width border takes no cells and draws nothing.
#[test]
fn a_zero_width_border_takes_no_cells() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(1)
        .set_border(Border::new(0, white()))
        .set_child(Text::new("hi"))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(6, 2), "hi\n\n");
  });
}

/// Bold is a plain `bool` on a run, and derived text re-renders on a signal change.
#[test]
fn bold_and_derived_text() {
  with_platform(|| {
    let count = Signal::new(1u32);

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Text::new("title").set_bold(true),
          Text::derive(move || format!("n={}", count.get())),
        ))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(8, 2), "title\nn=1\n");

    count.set(2);

    assert_eq!(tui::render_to_text(8, 2), "title\nn=2\n");
  });
}

/// Grid tracks size the columns: a fixed track, then the rest split evenly.
#[test]
fn grid_tracks_size_columns() {
  with_platform(|| {
    let _ = tui::mount(|| {
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

    assert_eq!(tui::render_to_text(12, 1), "a   b   c\n");
  });
}

/// A grid child placement spans tracks.
#[test]
fn grid_placement_spans_tracks() {
  with_platform(|| {
    let _ = tui::mount(|| {
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

    assert_eq!(tui::render_to_text(8, 2), "wide\nl   r\n");
  });
}

/// Flex grow distributes the free space between children.
#[test]
fn flex_grow_distributes_space() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Row::new()
        .set_children((
          Flexible::new().set_grow_factor(1.).set_child(
            Container::new()
              .set_color(Color::srgb(0.2, 0.2, 0.8))
              .set_child(Text::new("a")),
          ),
          Container::new().set_width(3).set_child(Text::new("b")),
        ))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(12, 1), "a        b\n");
  });
}

/// A fraction width resolves against the parent's extent.
#[test]
fn fraction_width_resolves_against_parent() {
  with_platform(|| {
    let _ = tui::mount(|| {
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

    assert_eq!(tui::render_to_text(12, 1), "half  x\n");
  });
}

/// A sized container fills its child to the content box; an unsized one hugs it.
#[test]
fn a_container_fills_when_sized_and_hugs_when_not() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(9)
        .set_height(3)
        .set_border(Border::new(1, white()))
        .set_child(Container::new().set_color(red()))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(9, 3, 1, 1).1, Some(rgb(red())));
    assert_eq!(colors_at(9, 3, 7, 1).1, Some(rgb(red())));
  });

  with_platform(|| {
    let _ = tui::mount(|| {
      Row::new()
        .set_item_alignment(ItemAlignment::Start)
        .set_child(
          Container::new()
            .set_border(Border::new(1, white()))
            .set_child(Text::new("hi")),
        )
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(9, 4), "┌──┐\n│hi│\n└──┘\n\n");
  });
}

/// A container places a child that fills neither axis by its alignment.
#[test]
fn a_container_aligns_its_child() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_alignment(Alignment::new(ItemAlignment::End, ItemAlignment::End))
        .set_child(Text::new("ab"))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(6, 3), "\n\n    ab\n");
  });
}

/// A layout property varies by condition on the value, so the terminal and the web
/// express responsiveness the same declarative way.
#[test]
fn layout_properties_vary_by_condition() {
  fn row() -> impl IntoView {
    Row::new()
      .set_spacing(Spacing::from(1).when(Media::MinWidth(10), Spacing::from(3)))
      .set_children((Text::new("ab"), Text::new("cd")))
  }

  with_platform(|| {
    let _ = tui::mount(|| row().into_view().build());

    assert_eq!(tui::render_to_text(12, 2), "ab   cd\n\n");
  });

  with_platform(|| {
    let _ = tui::mount(|| row().into_view().build());

    assert_eq!(tui::render_to_text(8, 2), "ab cd\n\n");
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
          direction: FlexDirection::Row,
        }),
      }
    }

    fn set_spacing(mut self, spacing: u32) -> Self {
      self.base = self.base.set_spacing(Spacing {
        row: 0,
        column: spacing,
      });

      self
    }
  }

  impl HasChildren<tui::Tui> for Toolbar {
    type Kind = FlexChild;

    fn children_mut(&mut self, seal: Seal) -> &mut Children<tui::Tui, FlexChild> {
      self.base.children_mut(seal)
    }
  }

  impl AnyKind for Toolbar {}

  impl skene_tui::core::View<tui::Tui> for Toolbar {
    fn build(self) -> tui::NodeId {
      skene_tui::core::View::build(self.base)
    }
  }

  let third_party = with_platform(|| {
    let _ = tui::mount(|| {
      Toolbar::new()
        .set_spacing(1)
        .set_children((
          Text::new("ab"),
          Flexible::new()
            .set_grow_factor(1.)
            .set_child(Text::new("cd")),
          Text::new("ef"),
        ))
        .into_any()
        .build()
    });

    tui::render_to_text(14, 3)
  });
  let built_in = with_platform(|| {
    let _ = tui::mount(|| {
      Row::new()
        .set_spacing(1)
        .set_children((
          Text::new("ab"),
          Flexible::new()
            .set_grow_factor(1.)
            .set_child(Text::new("cd")),
          Text::new("ef"),
        ))
        .into_any()
        .build()
    });

    tui::render_to_text(14, 3)
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

    let _ = tui::mount(|| {
      Container::new()
        .set_width(10)
        .set_child(
          RichText::new().set_spans([
            Span::new("see "),
            Span::view(
              Container::new()
                .set_padding(Inset::horizontal(1))
                .set_color(red())
                .handle_click(move || counted.set(counted.get() + 1))
                .set_child(Text::new("new")),
            ),
            Span::new(" here"),
          ]),
        )
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(12, 3);

    assert_eq!(frame, "see  new\nhere\n\n");

    let (column, row) = locate(&frame, "new");

    assert_eq!(column, 5);
    assert_eq!(colors_at(12, 3, 4, 0).1, Some(rgb(red())));
    assert_eq!(colors_at(12, 3, 8, 0).1, Some(rgb(red())));
    assert_eq!(colors_at(12, 3, 3, 0).1, None);
    assert_eq!(colors_at(12, 3, 9, 0).1, None);

    tui::trigger_mouse_click(column, row);

    assert_eq!(clicks.get(), 1);
  });
}
