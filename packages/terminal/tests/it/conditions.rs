//! Conditions in cells: the terminal's size, background and pointer as media, a named
//! container's size, a node's state over a derived slot, and a chosen scheme resolving
//! through tokens without a rebuild.

use std::{cell::Cell, rc::Rc};

use skene_simple_terminal::prelude::*;
use skene_terminal::{
  self as terminal,
  ContainerName,
  ContainerQuery,
  StaticLeaf,
  prelude::*,
  testing,
};

use crate::common::{blue, colors_at, green, red, rgb, white, with_launch, with_platform};

/// A `MaxWidth` variant applies at narrow terminal sizes and not at wide ones.
#[test]
fn media_variant_resolves_against_terminal_size() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_background(red().when(Media::max_width(10), blue()))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(8, 4, 0, 0).1, Some(rgb(blue())));
    assert_eq!(colors_at(20, 4, 0, 0).1, Some(rgb(red())));
  });
}

/// A composed condition resolves in the terminal: the variant applies when either
/// alternative holds, including a nested `and`.
#[test]
fn composed_condition_resolves() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_background(red().when(
          Condition::or([
            Media::max_width(10).into(),
            Condition::and([Media::min_height(6).into(), Media::max_height(8).into()]),
          ]),
          blue(),
        ))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(8, 4, 0, 0).1, Some(rgb(blue())), "narrow");
    assert_eq!(colors_at(20, 7, 0, 0).1, Some(rgb(blue())), "tall");
    assert_eq!(colors_at(20, 4, 0, 0).1, Some(rgb(red())), "wide");
  });
}

/// The scheme conditions follow the reported background: dark by default, and the
/// base value returns when the terminal reports a light one.
#[test]
fn color_scheme_conditions_follow_the_background() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_background(red().when(Media::Dark, blue()))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(12, 4, 0, 0).1, Some(rgb(blue())));

    testing::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(colors_at(12, 4, 0, 0).1, Some(rgb(red())));
  });
}

/// `media::scheme()` follows the reported background, and `media::watch` turns a
/// declared condition into a memo that moves with it.
#[test]
fn the_scheme_signal_and_a_watched_condition_follow_the_background() {
  with_platform(|| {
    let dark = terminal::media::watch(&Media::Dark);

    assert_eq!(terminal::media::scheme().get(), ColorScheme::Dark);
    assert!(dark.get());

    testing::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(terminal::media::scheme().get(), ColorScheme::Light);
    assert!(!dark.get());
  });
}

/// A border draws with the selected character set—rounded here, and ASCII through a
/// hover variant.
#[test]
fn border_glyphs_select_the_character_set() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(3)
        .set_border_width(Inset::all(1))
        .set_border_color(white())
        .set_border_glyphs(BorderGlyphs::ROUNDED.when(State::Hovered, BorderGlyphs::ASCII))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(6, 3), "╭──╮\n│  │\n╰──╯\n");

    testing::trigger_mouse_move(1, 1);

    assert_eq!(testing::render_to_text(6, 3), "+--+\n|  |\n+--+\n");
  });
}

/// An unconditional token ignores the terminal's background, so a subtree whose palette
/// names no condition paints the same however the terminal reports itself.
#[test]
fn an_unconditional_token_ignores_the_terminals_background() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Theme::new(SimplePalette::default().set_card(Color::srgb(0.2, 0.4, 0.8)))
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(1)
            .set_background(SimplePalette::CARD)
        })
        .into_any()
        .build()
    });

    let dark = testing::render_to_colors(4, 1);

    testing::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(dark, testing::render_to_colors(4, 1));
  });
}

/// A derived leaf may name a token: the box paints the palette's card until a write
/// switches the same slot to a fixed color.
#[test]
fn a_derived_token_paints_through_the_theme() {
  with_platform(|| {
    let plain = Signal::new(false);

    let _ = terminal::mount(move || {
      Theme::new(SimplePalette::default().set_card(blue()))
        .set_child_builder(move || {
          Container::new()
            .set_width(4)
            .set_height(1)
            .derive_background(move || {
              if plain.get() {
                StaticLeaf::from(red())
              }
              else {
                StaticLeaf::from(SimplePalette::CARD)
              }
            })
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(4, 1, 0, 0).1, Some(rgb(blue())));

    plain.set(true);

    assert_eq!(colors_at(4, 1, 0, 0).1, Some(rgb(red())));
  });
}

/// A chosen scheme re-inks the frame without rebuilding it: the paint reads the choice
/// where it read the detected background, so the nodes—and everything they hold—stay.
#[test]
fn a_derived_scheme_repaints_without_rebuilding() {
  with_platform(|| {
    let scheme = Signal::new(ColorScheme::Light);
    let builds = Rc::new(Cell::new(0u32));
    let counted = Rc::clone(&builds);

    let _ = terminal::mount(move || {
      Theme::new(SimplePalette::default())
        .set_child_builder(move || {
          derive_scheme(move || Some(scheme.get()));
          counted.set(counted.get() + 1);

          Container::new()
            .set_width(4)
            .set_height(1)
            .set_background(SimplePalette::CARD)
        })
        .into_any()
        .build()
    });

    let light = testing::render_to_colors(4, 1);

    assert_eq!(terminal::chosen_scheme(), Some(ColorScheme::Light));

    scheme.set(ColorScheme::Dark);

    assert_ne!(
      light,
      testing::render_to_colors(4, 1),
      "the choice reaches the paint"
    );
    assert_eq!(builds.get(), 1, "without building the subtree again");

    set_scheme(None);

    assert_eq!(terminal::chosen_scheme(), None);
  });
}

/// A derived slot is rewritten when its signal changes, and a hover variant on the same
/// property still wins over it while the node is hovered, whatever the slot then holds.
#[test]
fn a_derived_slot_updates_and_a_hover_variant_still_wins() {
  with_platform(|| {
    let color = Signal::new(red());

    let _ = terminal::mount(move || {
      Container::new()
        .set_width(4)
        .set_height(1)
        .set_background(
          Value::new(Leaf::derive(move || color.get()))
            .when(Condition::from(State::Hovered), blue()),
        )
        .into_any()
        .build()
    });
    let cell = || colors_at(6, 1, 0, 0).1;

    assert_eq!(cell(), Some(rgb(red())));

    color.set(green());

    assert_eq!(cell(), Some(rgb(green())));

    testing::trigger_mouse_move(1, 0);

    assert_eq!(cell(), Some(rgb(blue())), "the hover variant wins");

    color.set(red());

    assert_eq!(
      cell(),
      Some(rgb(blue())),
      "over a slot written while hovered"
    );

    testing::trigger_mouse_move(5, 0);

    assert_eq!(
      cell(),
      Some(rgb(red())),
      "the slot's value once the pointer leaves"
    );
  });
}

/// A container condition resolves against the named container's size, not the
/// terminal's: the box inside follows the container as its width changes in a terminal
/// of one width.
#[test]
fn a_container_condition_resolves_against_the_named_containers_size() {
  const SIDEBAR: ContainerName = ContainerName("sidebar");

  with_platform(|| {
    let width = Signal::new(20);

    let _ = terminal::mount(move || {
      Container::new()
        .set_container_name(SIDEBAR)
        .derive_width(move || width.get())
        .set_height(1)
        .set_child(
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(white().when(
              Media::Container {
                name: Some(SIDEBAR),
                query: ContainerQuery::min_width(30),
              },
              red(),
            )),
        )
        .into_any()
        .build()
    });
    let cell = || colors_at(80, 1, 0, 0).1;

    assert_eq!(
      cell(),
      Some(rgb(white())),
      "a sidebar narrower than the query"
    );

    width.set(40);

    assert_eq!(cell(), Some(rgb(red())), "one as wide");

    width.set(20);

    assert_eq!(cell(), Some(rgb(white())));
  });
}

/// The terminal's pointer is the mouse where the app takes it, fine and hovering, and
/// absent where the launch hands the mouse back: the signals, a watch and a property's
/// condition answer alike.
#[test]
fn the_pointer_answers_whether_the_app_takes_the_mouse() {
  fn mount() -> Memo<bool> {
    let hovers = Rc::new(Cell::new(None));

    let _ = terminal::mount({
      let hovers = Rc::clone(&hovers);

      move || {
        hovers.set(Some(terminal::media::watch(&Media::Hover)));

        Container::new()
          .set_width(2)
          .set_height(1)
          .set_background(red().when(Media::Pointer(PointerPrecision::Absent), blue()))
          .into_any()
          .build()
      }
    });

    hovers.get().expect("the watch")
  }

  with_platform(|| {
    let hovers = mount();

    assert_eq!(terminal::media::pointer().get(), PointerPrecision::Fine);
    assert!(terminal::media::hover().get());
    assert!(hovers.get());
    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(red())));
  });

  with_launch(terminal::Launch::new().set_mouse(false), || {
    let hovers = mount();

    assert_eq!(terminal::media::pointer().get(), PointerPrecision::Absent);
    assert!(!terminal::media::hover().get());
    assert!(!hovers.get());
    assert_eq!(colors_at(2, 1, 0, 0).1, Some(rgb(blue())));
  });
}
