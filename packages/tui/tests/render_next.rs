//! The terminal driver rendered headlessly: the built-in views laid out and painted to a
//! text grid, resolved through tokens and conditions, and driven through the seams.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::{cell::Cell, rc::Rc};

use skene_simple_tui::prelude::*;
use skene_tui::{
  self as tui,
  Activate,
  ContainerName,
  ContainerQuery,
  RadioMember,
  Roving,
  Scene,
  TextEntry,
  prelude::*,
};

use crate::common::{blue, colors_at, locate, red, with_driver};

/// A padded, bordered container paints its box-drawing border, in the border color.
#[test]
fn a_bordered_container_paints_its_border_in_the_border_color() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_padding(Inset::horizontal(1))
        .set_border(Border::new(1, red()))
        .set_child(Text::new("hi"))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(8, 3), "┌────┐\n│ hi │\n└────┘\n");
    assert_eq!(colors_at(8, 3, 0, 0).0, Some((255, 0, 0)));
    assert_eq!(colors_at(8, 3, 5, 2).0, Some((255, 0, 0)));
  });
}

/// A row lays its runs side by side, separated by its spacing, in cells.
#[test]
fn a_row_lays_its_children_out_in_cells() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Row::new()
        .set_spacing(2)
        .set_children((Text::new("ab"), Text::new("cd")))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(12, 2), "ab  cd\n\n");
  });
}

/// A theme resolves the card color on a card, and a palette flip re-inks the same node
/// without rebuilding: the build runs once and a signal inside keeps its value.
#[test]
fn a_theme_resolves_the_card_color_and_reinks_on_a_palette_flip() {
  with_driver(|| {
    let palette = Signal::opaque(SimplePalette::default().set_card(red()));
    let builds = Rc::new(Cell::new(0));
    let inner = Rc::new(Cell::new(None::<Signal<u32>>));
    let counted = Rc::clone(&builds);
    let kept = Rc::clone(&inner);
    let node = tui::mount(|| {
      Theme::derive(move || palette.get())
        .set_child_builder(move || {
          counted.set(counted.get() + 1);

          let count = Signal::new(1u32);

          kept.set(Some(count));

          SimpleCard::new().set_child(Text::derive(move || count.get().to_string()))
        })
        .into_any()
        .build()
    });
    let card = Scene::<tui::Tui>::with(|scene| scene.children(node).next()).unwrap();

    let _ = tui::render_to_text(20, 7);

    assert_eq!(colors_at(20, 7, 1, 1).1, Some((255, 0, 0)));

    inner.get().unwrap().set(2);
    palette.update(|palette| *palette = palette.clone().set_card(blue()));

    assert_eq!(colors_at(20, 7, 1, 1).1, Some((0, 0, 255)));
    assert_eq!(builds.get(), 1);
    assert_eq!(inner.get().unwrap().get(), 2);
    assert_eq!(
      Scene::<tui::Tui>::with(|scene| scene.children(node).next()),
      Some(card)
    );
    assert!(tui::render_to_text(20, 7).contains('2'));
  });
}

/// The detected background flips a token that answers the dark scheme.
#[test]
fn trigger_color_scheme_flips_a_dark_token() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Theme::new(SimplePalette::default())
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(1)
            .set_color(SimplePalette::CARD)
        })
        .into_any()
        .build()
    });

    tui::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some((255, 255, 255)));

    tui::trigger_color_scheme(ColorScheme::Dark);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some((24, 24, 27)));
  });
}

/// A button owns its click: the container around it never hears it, while a click on
/// the container's own cells does.
#[test]
fn a_button_click_does_not_reach_the_container_around_it() {
  with_driver(|| {
    let outer = Rc::new(Cell::new(0));
    let button = Rc::new(Cell::new(0));
    let outer_clicks = Rc::clone(&outer);
    let button_clicks = Rc::clone(&button);

    let _ = tui::mount(|| {
      Container::new()
        .set_padding(Inset::all(1))
        .handle_click(move || outer_clicks.set(outer_clicks.get() + 1))
        .set_child(SimpleButton::new("Go").handle_click(move || {
          button_clicks.set(button_clicks.get() + 1);
        }))
        .into_any()
        .build()
    });

    let frame = tui::render_to_text(30, 8);
    let (x, y) = locate(&frame, "Go");

    tui::trigger_mouse_click(x, y);

    assert_eq!((button.get(), outer.get()), (1, 0));

    tui::trigger_mouse_click(0, 0);

    assert_eq!((button.get(), outer.get()), (1, 1));
  });
}

/// `Space` on a focused checkbox flips the bound signal, and the mark repaints as the
/// checked glyph.
#[test]
fn space_on_a_focused_checkbox_flips_the_signal_and_repaints_the_mark() {
  with_driver(|| {
    let agreed = Signal::new(false);

    let _ = tui::mount(|| {
      Column::new()
        .set_child(
          SimpleCheckbox::new()
            .bind_checked(agreed)
            .set_label("agree"),
        )
        .into_any()
        .build()
    });

    assert!(tui::render_to_text(20, 5).starts_with("[ ] agree"));

    tui::trigger_key("Tab");
    tui::trigger_key(" ");

    assert!(agreed.get());
    assert!(tui::render_to_text(20, 5).starts_with("[x] agree"));
  });
}

/// `Tab` reaches the controls in document order, and a radio group with nothing chosen
/// is reachable through its first member.
#[test]
fn tab_reaches_the_controls_in_document_order() {
  with_driver(|| {
    let first = Anchor::new();
    let mint = Anchor::new();
    let last = Anchor::new();

    let _ = tui::mount({
      let (first, mint, last) = (first.clone(), mint.clone(), last.clone());

      move || {
        Column::new()
          .set_children((
            RawButton::new(Activate::new())
              .bind_anchor(&first)
              .set_label("first")
              .into_any(),
            RawRadioGroup::new(Roving::new())
              .set_child(
                Row::new().set_spacing(1).set_children((
                  RawRadio::new(RadioMember::new("mint"))
                    .bind_anchor(&mint)
                    .set_label("mint"),
                  RawRadio::new(RadioMember::new("plum")).set_label("plum"),
                )),
              )
              .into_any(),
            RawButton::new(Activate::new())
              .bind_anchor(&last)
              .set_label("last")
              .into_any(),
          ))
          .into_any()
          .build()
      }
    });
    let _ = tui::render_to_text(30, 6);

    tui::trigger_key("Tab");

    assert_eq!(tui::focused_node(), first.node());

    tui::trigger_key("Tab");

    assert_eq!(tui::focused_node(), mint.node());

    tui::trigger_key("Tab");

    assert_eq!(tui::focused_node(), last.node());

    tui::trigger_key("Shift+Tab");

    assert_eq!(tui::focused_node(), mint.node());
  });
}

/// A container query in cells switches a padding when the terminal narrows.
#[test]
fn a_container_query_switches_a_padding_when_the_terminal_narrows() {
  const CARD: ContainerName = ContainerName("card");

  with_driver(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_container_name(CARD)
        .set_child(
          Container::new()
            .set_padding(Inset::all(0).when(
              Media::Container {
                name: Some(CARD),
                query: ContainerQuery::MaxWidth(30),
              },
              Inset::all(2),
            ))
            .set_child(Text::new("x")),
        )
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(80, 5).lines().next(), Some("x"));

    tui::trigger_resize(20, 5);

    assert_eq!(tui::render_to_text(20, 5).lines().nth(2), Some("  x"));
  });
}

/// A hover change repaints without a relayout: the hover variant's color paints, and
/// the layout generation stays where it was.
#[test]
fn a_hover_change_repaints_without_a_relayout() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(10)
        .set_height(3)
        .set_color(red().when(State::Hovered, blue()))
        .set_child(Text::new("hover"))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(12, 4, 1, 1).1, Some((255, 0, 0)));

    let generation = tui::layout_generation();
    let writes = tui::set_style_calls();

    tui::trigger_mouse_move(1, 1);

    assert_eq!(colors_at(12, 4, 1, 1).1, Some((0, 0, 255)));
    assert_eq!(tui::layout_generation(), generation);
    assert_eq!(tui::set_style_calls(), writes);
  });
}

/// A keystroke in a text entry restyles the entry alone.
#[test]
fn a_keystroke_restyles_only_the_changed_entry() {
  with_driver(|| {
    let name = Signal::new("abcdefgh".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Text::new("label"),
          RawTextField::new(TextEntry::new().bind_value(name)),
          Text::new("after"),
        ))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(30, 5);
    tui::trigger_key("Tab");
    let _ = tui::render_to_text(30, 5);

    let writes = tui::set_style_calls();

    tui::trigger_key("i");
    let _ = tui::render_to_text(30, 5);

    assert_eq!(name.get(), "abcdefghi");
    assert_eq!(tui::set_style_calls(), writes + 1);
  });
}

/// A corner radius rounds the border: the terminal's radius is the arc glyph set.
#[test]
fn a_corner_radius_renders_arc_corners() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(3)
        .set_border(Border::new(1, red()))
        .set_corner_radius(1.)
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(5, 3), "╭──╮\n│  │\n╰──╯\n");
  });
}

/// A text entry types through the key seam and writes the bound signal, the caret
/// following.
#[test]
fn a_text_field_types_through_trigger_key_and_writes_the_bound_signal() {
  with_driver(|| {
    let name = Signal::new(String::new());

    let _ = tui::mount(|| {
      RawTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = tui::render_to_text(20, 2);
    tui::trigger_key("Tab");
    tui::trigger_key("h");
    tui::trigger_key("i");

    assert_eq!(name.get(), "hi");
    assert_eq!(tui::render_to_text(20, 2).lines().next(), Some("hi"));
    assert_eq!(tui::render_to_caret(20, 2), Some((2, 0)));

    tui::trigger_key("ArrowLeft");
    tui::trigger_key("Backspace");

    assert_eq!(name.get(), "i");
  });
}

/// An overlay opens above the content and `Escape` asks it to close.
#[test]
fn an_overlay_opens_above_content_and_dismisses_on_escape() {
  with_driver(|| {
    let open = Signal::new(false);

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Text::new("under").into_any(),
          Overlay::modal()
            .bind_open(open)
            .set_backdrop(None)
            .set_child(Text::new("modal"))
            .into_any(),
        ))
        .into_any()
        .build()
    });

    assert!(!tui::render_to_text(20, 5).contains("modal"));

    open.set(true);

    let frame = tui::render_to_text(20, 5);

    assert!(frame.contains("modal"), "{frame}");
    assert!(frame.contains("under"), "{frame}");

    tui::trigger_key("Escape");

    assert!(!open.get());
    assert!(!tui::render_to_text(20, 5).contains("modal"));
  });
}
