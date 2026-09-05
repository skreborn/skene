//! Motion on the terminal's frame clock: a transition steps across frames from every
//! change source, reduced motion cuts, and a keyframe animation samples its frames.

#![cfg(not(target_arch = "wasm32"))]

mod common;

use std::time::Duration;

use skene_tui::{
  self as tui,
  Color,
  State,
  core::{BoxKeyframe, Property},
  prelude::*,
};

use crate::common::{blue, colors_at, red, rgb, with_driver};

/// A one-second linear color transition.
fn color_transition() -> Transition<tui::TerminalProperty> {
  Transition::new(Property::Color, 1.).set_easing(Easing::Linear)
}

/// A box keyframe of one background color.
fn frame(color: impl Into<skene_tui::StaticLeaf<Color>>) -> BoxKeyframe<tui::Tui> {
  BoxKeyframe::<tui::Tui> {
    color: Some(color.into()),
    ..Default::default()
  }
}

/// A hover flipping a color under a transition paints the start color on the frame of
/// the flip, the Oklab midpoint half a second in, and the end color once settled.
#[test]
fn a_hover_transition_steps_a_color_across_frames() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Container::new()
        .set_width(4)
        .set_height(1)
        .set_color(red().when(State::Hovered, blue()))
        .set_transition(color_transition())
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    tui::trigger_mouse_move(0, 0);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    tui::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    tui::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    tui::trigger_frame(Duration::from_secs(1));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));
  });
}

/// A derived width moving under a transition lays out at the in-between cell count,
/// rounded, and lands on its target.
#[test]
fn a_derived_length_transition_rounds_to_cells() {
  with_driver(|| {
    let width = Signal::new(2u32);
    let _ = tui::mount(move || {
      Container::new()
        .derive_width(move || Dimension::Length(width.get()))
        .set_height(1)
        .set_color(red())
        .set_transition(Transition::new(Property::Width, 1.).set_easing(Easing::Linear))
        .into_any()
        .build()
    });
    let painted = |x: usize| colors_at(12, 2, x, 0).1 == Some(rgb(red()));

    assert!(painted(1));
    assert!(!painted(2));

    width.set(10);

    assert!(painted(1));
    assert!(!painted(2));

    tui::trigger_frame(Duration::from_millis(500));

    assert!(painted(5));
    assert!(!painted(6));

    tui::trigger_frame(Duration::from_millis(500));

    assert!(painted(9));
    assert!(!painted(10));
  });
}

/// Reduced motion cuts a transition that respects it and leaves one that opted out
/// running.
#[test]
fn reduced_motion_cuts_a_respecting_transition() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Row::new()
        .set_children((
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_color(red().when(State::Hovered, blue()))
            .set_transition(color_transition()),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_color(red().when(State::Hovered, blue()))
            .set_transition(color_transition().set_respect_reduced_motion(false)),
        ))
        .into_any()
        .build()
    });

    tui::trigger_reduced_motion(true);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    tui::trigger_mouse_move(0, 0);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    tui::trigger_mouse_move(2, 0);

    assert_eq!(colors_at(6, 2, 2, 0).1, Some(rgb(red())));

    tui::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 2, 0).1, Some(rgb(red().mix(blue(), 0.5))));
  });
}

/// A palette flip moves a token-driven color through its transition without touching
/// the node.
#[test]
fn a_palette_flip_transitions_a_token_driven_color() {
  with_driver(|| {
    let palette = Signal::opaque(SimplePalette::default().set_card(red()));
    let _ = tui::mount(move || {
      Theme::derive(move || palette.get())
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(1)
            .set_color(SimplePalette::CARD)
            .set_transition(color_transition())
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    palette.update(|palette| *palette = palette.clone().set_card(blue()));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    tui::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    tui::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));
  });
}

/// A keyframe animation samples between its frames on the clock and rests on its last
/// frame; its token leaves resolve through the theme.
#[test]
fn a_keyframe_animation_samples_its_frames() {
  with_driver(|| {
    let _ = tui::mount(|| {
      Theme::new(SimplePalette::default().set_card(blue()))
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(1)
            .set_color(red())
            .set_animation(
              Animation::new(1., [(0., frame(red())), (100., frame(SimplePalette::CARD))])
                .set_easing(Easing::Linear),
            )
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    tui::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    tui::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    tui::trigger_frame(Duration::from_secs(2));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));
  });
}
