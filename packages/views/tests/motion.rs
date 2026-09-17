//! Motion on the host platform: a transition samples across the stepped clock from a
//! derived write and a palette flip, reduced motion cuts, a keyframe animation resolves
//! its token leaves, and a transition name reaches the props.

mod common;

use std::time::Duration;

use skene_core::{Animation, BoxKeyframe, Color, Easing, Property, Transition, View};
use skene_headless::{
  Headless,
  advance,
  box_props,
  children,
  reset,
  resolve_color,
  resolve_length,
  set_reduced_motion,
};
use skene_reactive::{Get, Owner, Set, Signal};
use skene_views::{Container, Theme};

use crate::common::{COLOR, TestPalette, WIDTH, blue, red};

/// A one-second linear transition of `property`.
fn linear(property: Property) -> Transition {
  Transition::new(property, 1.).set_easing(Easing::Linear)
}

/// A box keyframe of one background color leaf.
fn frame(color: impl Into<skene_core::StaticLeaf<Color>>) -> BoxKeyframe<Headless> {
  BoxKeyframe::<Headless> {
    color: Some(color.into()),
    ..Default::default()
  }
}

/// A derived color and width moving under transitions resolve to the in-between value
/// at the stepped clock, and land on their targets.
#[test]
fn derived_writes_transition_across_the_stepped_clock() {
  reset();

  let owner = Owner::new();
  let color = Signal::new(red());
  let width = Signal::new(0f32);
  let node = owner.enter(|| {
    Container::<Headless>::new()
      .derive_color(move || color.get())
      .derive_width(move || width.get())
      .set_transition(linear(Property::Color))
      .set_transition(linear(Property::Width))
      .build()
  });

  assert_eq!(resolve_color(node, COLOR), Some(red()));
  assert_eq!(resolve_length(node, WIDTH), Some(0.));

  color.set(blue());
  width.set(10.);
  advance(Duration::from_millis(500));

  assert_eq!(resolve_color(node, COLOR), Some(red().mix(blue(), 0.5)));
  assert_eq!(resolve_length(node, WIDTH), Some(5.));

  advance(Duration::from_millis(500));

  assert_eq!(resolve_color(node, COLOR), Some(blue()));
  assert_eq!(resolve_length(node, WIDTH), Some(10.));

  owner.dispose();
}

/// A palette flip moves a token-driven color through its transition; reduced motion
/// cuts a transition that respects it.
#[test]
fn a_palette_flip_transitions_and_reduced_motion_cuts() {
  reset();

  let owner = Owner::new();
  let palette = Signal::opaque(TestPalette::with_card(red()));
  let root = owner.enter(|| {
    Theme::<Headless, TestPalette>::derive(move || palette.get())
      .set_child_builder(|| {
        Container::new()
          .set_color(TestPalette::CARD)
          .set_transition(linear(Property::Color))
      })
      .build()
  });
  let card = children(root)[0];

  assert_eq!(resolve_color(card, COLOR), Some(red()));

  palette.set(TestPalette::with_card(blue()));
  advance(Duration::from_millis(500));

  assert_eq!(resolve_color(card, COLOR), Some(red().mix(blue(), 0.5)));

  advance(Duration::from_millis(500));

  assert_eq!(resolve_color(card, COLOR), Some(blue()));

  set_reduced_motion(true);
  palette.set(TestPalette::with_card(red()));
  advance(Duration::from_millis(1));

  assert_eq!(resolve_color(card, COLOR), Some(red()));

  set_reduced_motion(false);
  owner.dispose();
}

/// A keyframe animation resolves its token leaf through the theme, samples between its
/// frames, and rests on its last frame; the transition name reaches the props.
#[test]
fn a_token_keyframe_animation_samples_through_the_theme() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Theme::<Headless, TestPalette>::new(TestPalette::with_card(blue()))
      .set_child_builder(|| {
        Container::new()
          .set_color(red())
          .set_transition_name("hero")
          .set_animation(
            Animation::new(1., [(0., frame(red())), (100., frame(TestPalette::CARD))])
              .set_easing(Easing::Linear),
          )
      })
      .build()
  });
  let card = children(root)[0];

  assert_eq!(
    box_props(card).unwrap().transition_name.as_deref(),
    Some("hero")
  );
  assert_eq!(resolve_color(card, COLOR), Some(red()));

  advance(Duration::from_millis(500));

  assert_eq!(resolve_color(card, COLOR), Some(red().mix(blue(), 0.5)));

  advance(Duration::from_secs(1));

  assert_eq!(resolve_color(card, COLOR), Some(blue()));

  owner.dispose();
}
