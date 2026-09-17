//! Conditions in cells: the terminal's size and background as media, a node's state,
//! and a chosen scheme resolving through tokens without a rebuild.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::{cell::Cell, rc::Rc};

use skene_simple::tui::prelude::*;
use skene_tui::{self as tui, StaticLeaf, prelude::*};

use crate::common::{blue, colors_at, red, rgb, white, with_platform};

/// A `MaxWidth` variant applies at narrow terminal sizes and not at wide ones.
#[test]
fn media_variant_resolves_against_terminal_size() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_color(red().when(Media::MaxWidth(10), blue()))
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
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_color(red().when(
          Condition::or([
            Media::MaxWidth(10).into(),
            Condition::and([Media::MinHeight(6).into(), Media::MaxHeight(8).into()]),
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
    let _ = tui::mount(|| {
      Container::new()
        .set_width(6)
        .set_height(3)
        .set_color(red().when(Media::Dark, blue()))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(12, 4, 0, 0).1, Some(rgb(blue())));

    tui::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(colors_at(12, 4, 0, 0).1, Some(rgb(red())));
  });
}

/// `media::scheme()` follows the reported background, and `media::watch` turns a
/// declared condition into a memo that moves with it.
#[test]
fn the_scheme_signal_and_a_watched_condition_follow_the_background() {
  with_platform(|| {
    let dark = tui::media::watch(&Media::Dark);

    assert_eq!(tui::media::scheme().get(), ColorScheme::Dark);
    assert!(dark.get());

    tui::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(tui::media::scheme().get(), ColorScheme::Light);
    assert!(!dark.get());
  });
}

/// A border draws with the selected character set—rounded here, and ASCII through a
/// hover variant.
#[test]
fn border_glyphs_select_the_character_set() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(3)
        .set_border(Border::new(1, white()))
        .set_border_glyphs(BorderGlyphs::ROUNDED.when(State::Hovered, BorderGlyphs::ASCII))
        .into_any()
        .build()
    });

    assert_eq!(tui::render_to_text(6, 3), "╭──╮\n│  │\n╰──╯\n");

    tui::trigger_mouse_move(1, 1);

    assert_eq!(tui::render_to_text(6, 3), "+--+\n|  |\n+--+\n");
  });
}

/// An unconditional token ignores the terminal's background, so a subtree whose palette
/// names no condition paints the same however the terminal reports itself.
#[test]
fn an_unconditional_token_ignores_the_terminals_background() {
  with_platform(|| {
    let _ = tui::mount(|| {
      Theme::new(SimplePalette::default().set_card(Color::srgb(0.2, 0.4, 0.8)))
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(1)
            .set_color(SimplePalette::CARD)
        })
        .into_any()
        .build()
    });

    let dark = tui::render_to_colors(4, 1);

    tui::trigger_color_scheme(ColorScheme::Light);

    assert_eq!(dark, tui::render_to_colors(4, 1));
  });
}

/// A derived leaf may name a token: the box paints the palette's card until a write
/// switches the same slot to a fixed color.
#[test]
fn a_derived_token_paints_through_the_theme() {
  with_platform(|| {
    let plain = Signal::new(false);

    let _ = tui::mount(move || {
      Theme::new(SimplePalette::default().set_card(blue()))
        .set_child_builder(move || {
          Container::new()
            .set_width(4)
            .set_height(1)
            .derive_color(move || {
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

    let _ = tui::mount(move || {
      Theme::new(SimplePalette::default())
        .set_child_builder(move || {
          derive_scheme(move || Some(scheme.get()));
          counted.set(counted.get() + 1);

          Container::new()
            .set_width(4)
            .set_height(1)
            .set_color(SimplePalette::CARD)
        })
        .into_any()
        .build()
    });

    let light = tui::render_to_colors(4, 1);

    assert_eq!(tui::stored_scheme(), Some(ColorScheme::Light));

    scheme.set(ColorScheme::Dark);

    assert_ne!(
      light,
      tui::render_to_colors(4, 1),
      "the choice reaches the paint"
    );
    assert_eq!(builds.get(), 1, "without building the subtree again");

    set_scheme(None);

    assert_eq!(tui::stored_scheme(), None);
  });
}
