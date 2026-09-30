//! The terminal platform rendered headlessly: the built-in views laid out and painted to a
//! text grid, resolved through tokens and conditions, and driven through the seams.

use std::{cell::Cell, rc::Rc};

use skene_simple::tui::prelude::*;
use skene_tui::{
  self as tui,
  Activate,
  ContainerName,
  ContainerQuery,
  Radio,
  RadioGroup,
  Scene,
  TextEntry,
  prelude::*,
  testing,
};

use crate::common::{blue, colors_at, locate, red, with_platform};

/// A padded, bordered container paints its box-drawing border, in the border color.
#[test]
fn a_bordered_container_paints_its_border_in_the_border_color() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_padding(Inset::horizontal(1))
        .set_border_width(Inset::all(1))
        .set_border_color(red())
        .set_child(Text::new("hi"))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(8, 3), "┌────┐\n│ hi │\n└────┘\n");
    assert_eq!(colors_at(8, 3, 0, 0).0, Some((255, 0, 0)));
    assert_eq!(colors_at(8, 3, 5, 2).0, Some((255, 0, 0)));
  });
}

/// A row lays its runs side by side, separated by its spacing, in cells.
#[test]
fn a_row_lays_its_children_out_in_cells() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Row::new()
        .set_spacing(2)
        .set_children((Text::new("ab"), Text::new("cd")))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(12, 2), "ab  cd\n\n");
  });
}

/// A theme resolves the card color on a card, and a palette flip re-inks the same node
/// without rebuilding: the build runs once and a signal inside keeps its value.
#[test]
fn a_theme_resolves_the_card_color_and_reinks_on_a_palette_flip() {
  with_platform(|| {
    let palette = Signal::transient(SimplePalette::default().set_card(red()));
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

          SimpleCard::new().add_child(Text::derive(move || count.get().to_string()))
        })
        .into_any()
        .build()
    });
    let card = Scene::<tui::Tui>::with(|scene| scene.children(node).next()).unwrap();

    let _ = testing::render_to_text(20, 7);

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
    assert!(testing::render_to_text(20, 7).contains('2'));
  });
}

/// The detected background flips a token that answers the dark scheme.
#[test]
fn trigger_color_scheme_flips_a_dark_token() {
  with_platform(|| {
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

    testing::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some((255, 255, 255)));

    testing::trigger_color_scheme(ColorScheme::Dark);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some((24, 24, 27)));
  });
}

/// A button owns its click: the container around it never hears it, while a click on
/// the container's own cells does.
#[test]
fn a_button_click_does_not_reach_the_container_around_it() {
  with_platform(|| {
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

    let frame = testing::render_to_text(30, 8);
    let (x, y) = locate(&frame, "Go");

    testing::trigger_mouse_click(x, y);

    assert_eq!((button.get(), outer.get()), (1, 0));

    testing::trigger_mouse_click(0, 0);

    assert_eq!((button.get(), outer.get()), (1, 1));
  });
}

/// `Space` on a focused checkbox flips the bound signal, and the mark repaints as the
/// checked glyph.
#[test]
fn space_on_a_focused_checkbox_flips_the_signal_and_repaints_the_mark() {
  with_platform(|| {
    let agreed = Signal::new(false);

    let _ = tui::mount(|| {
      Column::new()
        .add_child(
          SimpleCheckbox::new()
            .bind_checked(agreed)
            .set_label("agree"),
        )
        .into_any()
        .build()
    });

    assert!(testing::render_to_text(20, 5).starts_with("[ ] agree"));

    testing::trigger_key("Tab");
    testing::trigger_key(" ");

    assert!(agreed.get());
    assert!(testing::render_to_text(20, 5).starts_with("[x] agree"));
  });
}

/// `Tab` reaches the controls in document order, and a radio group with nothing chosen
/// is reachable through its first member.
#[test]
fn tab_reaches_the_controls_in_document_order() {
  with_platform(|| {
    let first = NodeHandle::new();
    let mint = NodeHandle::new();
    let last = NodeHandle::new();

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          BasicButton::new(Activate::new())
            .bind_handle(first)
            .set_label("first")
            .into_any(),
          BasicRadioGroup::new(RadioGroup::new())
            .set_child(
              Row::new().set_spacing(1).set_children((
                BasicRadio::new(Radio::new("mint"))
                  .bind_handle(mint)
                  .set_label("mint"),
                BasicRadio::new(Radio::new("plum")).set_label("plum"),
              )),
            )
            .into_any(),
          BasicButton::new(Activate::new())
            .bind_handle(last)
            .set_label("last")
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 6);

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), first.node());

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), mint.node());

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), last.node());

    testing::trigger_key("Shift+Tab");

    assert_eq!(testing::focused_node(), mint.node());
  });
}

/// A container query in cells switches a padding when the terminal narrows.
#[test]
fn a_container_query_switches_a_padding_when_the_terminal_narrows() {
  const CARD: ContainerName = ContainerName("card");

  with_platform(|| {
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

    assert_eq!(testing::render_to_text(80, 5).lines().next(), Some("x"));

    testing::trigger_resize(20, 5);

    assert_eq!(testing::render_to_text(20, 5).lines().nth(2), Some("  x"));
  });
}

/// A hover change repaints without a relayout: the hover variant's color paints, and
/// the layout generation stays where it was.
#[test]
fn a_hover_change_repaints_without_a_relayout() {
  with_platform(|| {
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

    let generation = testing::layout_generation();
    let writes = testing::style_writes();

    testing::trigger_mouse_move(1, 1);

    assert!(
      testing::is_frame_pending(),
      "the hover change schedules a frame"
    );
    assert_eq!(colors_at(12, 4, 1, 1).1, Some((0, 0, 255)));
    assert_eq!(testing::layout_generation(), generation);
    assert_eq!(testing::style_writes(), writes);
  });
}

/// A hover resolves again only the nodes whose conditions read it: moving over a plain
/// box resolves nothing, and entering and leaving a box with a hover variant resolves
/// that box alone, once each way.
#[test]
fn a_hover_resolves_only_the_nodes_reading_it() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Container::new().set_height(1).set_child(Text::new("plain")),
          Container::new()
            .set_height(1)
            .set_color(red().when(State::Hovered, blue()))
            .set_child(Text::new("hover")),
          Container::new().set_height(1).set_child(Text::new("plain")),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(10, 3);
    let resolved = testing::style_resolutions();

    testing::trigger_mouse_move(1, 0);
    let _ = testing::render_to_text(10, 3);

    assert_eq!(testing::style_resolutions(), resolved);

    testing::trigger_mouse_move(1, 1);
    let _ = testing::render_to_text(10, 3);

    assert_eq!(testing::style_resolutions(), resolved + 1);

    testing::trigger_mouse_move(1, 2);
    let _ = testing::render_to_text(10, 3);

    assert_eq!(testing::style_resolutions(), resolved + 2);
  });
}

/// A frame with nothing changed since the last one resolves nothing and repaints no
/// row.
#[test]
fn an_unchanged_frame_repaints_nothing() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Column::new()
        .set_children((Text::new("one"), Text::new("two")))
        .into_any()
        .build()
    });
    let first = testing::render_to_text(10, 3);
    let resolved = testing::style_resolutions();

    assert_eq!(testing::render_to_text(10, 3), first);
    assert_eq!(testing::style_resolutions(), resolved);
    assert_eq!(testing::repainted_rows(), Vec::<usize>::new());
  });
}

/// A keystroke in a text entry restyles the entry alone.
#[test]
fn a_keystroke_restyles_only_the_changed_entry() {
  with_platform(|| {
    let name = Signal::new("abcdefgh".to_owned());

    let _ = tui::mount(|| {
      Column::new()
        .set_children((
          Text::new("label"),
          BasicTextField::new(TextEntry::new().bind_value(name)),
          Text::new("after"),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(30, 5);
    testing::trigger_key("Tab");
    testing::trigger_key("End");
    let _ = testing::render_to_text(30, 5);

    let writes = testing::style_writes();

    testing::trigger_key("i");
    let _ = testing::render_to_text(30, 5);

    assert_eq!(name.get(), "abcdefghi");
    assert_eq!(testing::style_writes(), writes + 1);
  });
}

/// A corner radius rounds the border: the terminal's radius is the arc glyph set.
#[test]
fn a_corner_radius_renders_arc_corners() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(3)
        .set_border_width(Inset::all(1))
        .set_border_color(red())
        .set_corner_radius(1.)
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(5, 3), "╭──╮\n│  │\n╰──╯\n");
  });
}

/// A text entry types through the key seam and writes the bound signal, the caret
/// following.
#[test]
fn a_text_field_types_through_trigger_key_and_writes_the_bound_signal() {
  with_platform(|| {
    let name = Signal::new(String::new());

    let _ = tui::mount(|| {
      BasicTextField::new(TextEntry::new().bind_value(name))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 2);
    testing::trigger_key("Tab");
    testing::trigger_key("h");
    testing::trigger_key("i");

    assert_eq!(name.get(), "hi");
    assert_eq!(testing::render_to_text(20, 2).lines().next(), Some("hi"));
    assert_eq!(testing::render_to_caret(20, 2), Some((2, 0)));

    testing::trigger_key("ArrowLeft");
    testing::trigger_key("Backspace");

    assert_eq!(name.get(), "i");
  });
}

/// An overlay opens above the content and `Escape` asks it to close.
#[test]
fn an_overlay_opens_above_content_and_dismisses_on_escape() {
  with_platform(|| {
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

    assert!(!testing::render_to_text(20, 5).contains("modal"));

    open.set(true);

    let frame = testing::render_to_text(20, 5);

    assert!(frame.contains("modal"), "{frame}");
    assert!(frame.contains("under"), "{frame}");

    testing::trigger_key("Escape");

    assert!(!open.get());
    assert!(!testing::render_to_text(20, 5).contains("modal"));
  });
}

/// A key release the terminal reports reaches the focused box's key-up handler, and a
/// modifier pressed alone reaches the key-down handler as its W3C name without showing
/// focus.
#[test]
fn a_key_release_and_a_bare_modifier_reach_their_handlers() {
  with_platform(|| {
    let released = Signal::new(String::new());
    let pressed = Signal::new(String::new());

    let _ = tui::mount(move || {
      Container::new()
        .handle_key_down(move |event| pressed.set(event.key.to_string()))
        .handle_key_up(move |event| released.set(event.key.to_string()))
        .set_child(BasicButton::new(Activate::new()).set_label("press"))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(20, 2);

    testing::trigger_key_releases(true);
    testing::trigger_key("Tab");

    assert_eq!(released.get_untracked(), "");

    testing::trigger_key_up("Tab");

    assert_eq!(released.get_untracked(), "Tab");

    testing::trigger_key("Shift");

    assert_eq!(pressed.get_untracked(), "Shift");
  });
}

/// A hidden box keeps its place but paints nothing, its content with it, and takes
/// neither the pointer nor focus; shown again, it paints and takes both.
#[test]
fn a_hidden_box_keeps_its_place_and_paints_nothing() {
  with_platform(|| {
    let shown = Signal::new(false);
    let clicks = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&clicks);

    let _ = tui::mount(move || {
      Column::new()
        .set_children((
          Container::new()
            .derive_visibility(move || {
              if shown.get() {
                Visibility::Visible
              }
              else {
                Visibility::Hidden
              }
            })
            .set_child(
              BasicButton::new(Activate::new())
                .set_label("hidden")
                .handle_click(move || counted.set(counted.get() + 1)),
            ),
          Text::new("below"),
        ))
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(10, 2);

    assert!(!frame.contains("hidden"), "{frame}");
    assert!(
      frame
        .lines()
        .nth(1)
        .is_some_and(|line| line.contains("below")),
      "the box keeps its place: {frame}"
    );

    testing::trigger_mouse_click(1, 0);
    testing::trigger_key("Tab");
    testing::trigger_key("Enter");

    assert_eq!(
      clicks.get(),
      0,
      "neither the pointer nor the keyboard reaches it"
    );

    shown.set(true);

    assert!(testing::render_to_text(10, 2).contains("hidden"));

    testing::trigger_mouse_click(1, 0);

    assert_eq!(clicks.get(), 1);
  });
}
