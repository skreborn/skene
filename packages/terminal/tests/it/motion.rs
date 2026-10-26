//! Motion on the terminal's frame clock: a transition steps across frames from every
//! change source, reduced motion cuts, an interrupted transition reverses over what it
//! played, and a keyframe animation samples its frames, moving from the resolved value
//! where a frame is missing.

use std::time::Duration;

use skene_foundation::{BoxKeyframe, Property, RunKeyframe};
use skene_simple_terminal::prelude::*;
use skene_terminal::{
  Color,
  State,
  platform::terminal::{Terminal, mount},
  prelude::*,
  testing,
};

use crate::common::{blue, colors_at, red, rgb, with_platform};

/// A one-second linear color transition.
fn color_transition() -> Transition<skene_terminal::TerminalProperty> {
  Transition::new(Property::Background, 1.).set_easing(Easing::Linear)
}

/// A box keyframe of one background color.
fn frame(color: impl Into<skene_terminal::StaticLeaf<Color>>) -> BoxKeyframe<Terminal> {
  BoxKeyframe::<Terminal> {
    background: Some(color.into()),
    ..Default::default()
  }
}

/// A run keyframe of one text color.
fn ink_frame(color: impl Into<skene_terminal::StaticLeaf<Color>>) -> RunKeyframe<Terminal> {
  RunKeyframe::<Terminal> {
    color: Some(color.into()),
    ..Default::default()
  }
}

/// A hover flipping a color under a transition paints the start color on the frame of
/// the flip, the Oklab midpoint half a second in, and the end color once settled.
#[test]
fn a_hover_transition_steps_a_color_across_frames() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_width(4)
        .set_height(1)
        .set_background(red().when(State::Hovered, blue()))
        .add_transition(color_transition())
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    testing::trigger_mouse_move(0, 0);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    testing::trigger_frame(Duration::from_secs(1));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));
  });
}

/// A derived width moving under a transition lays out at the in-between cell count,
/// rounded, and lands on its target.
#[test]
fn a_derived_length_transition_rounds_to_cells() {
  with_platform(|| {
    let width = Signal::new(2i32);
    let _ = mount(move || {
      Container::new()
        .derive_width(move || Dimension::Length(width.get()))
        .set_height(1)
        .set_background(red())
        .add_transition(Transition::new(Property::Width, 1.).set_easing(Easing::Linear))
        .into_any()
        .build()
    });
    let painted = |x: usize| colors_at(12, 2, x, 0).1 == Some(rgb(red()));

    assert!(painted(1));
    assert!(!painted(2));

    width.set(10);

    assert!(painted(1));
    assert!(!painted(2));

    testing::trigger_frame(Duration::from_millis(500));

    assert!(painted(5));
    assert!(!painted(6));

    testing::trigger_frame(Duration::from_millis(500));

    assert!(painted(9));
    assert!(!painted(10));
  });
}

/// A derived color moving under a transition paints the in-between color at the
/// stepped clock and lands on its target.
#[test]
fn a_derived_color_transitions_across_the_stepped_clock() {
  with_platform(|| {
    let color = Signal::new(red());
    let _ = mount(move || {
      Container::new()
        .set_width(4)
        .set_height(1)
        .derive_background(move || color.get())
        .add_transition(color_transition())
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    color.set(blue());

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));
  });
}

/// A write back to where a running transition began reverses it over the share it had
/// played, as CSS's reversing shortening does: a quarter of the way to eight cells, it
/// returns to none in a quarter of the duration.
#[test]
fn an_interrupted_transition_reverses_over_what_it_played() {
  with_platform(|| {
    let width = Signal::new(0i32);
    let _ = mount(move || {
      Column::new()
        .set_children((Container::new()
          .derive_width(move || Dimension::Length(width.get()))
          .set_height(1)
          .set_background(red())
          .add_transition(Transition::new(Property::Width, 1.).set_easing(Easing::Linear)),))
        .into_any()
        .build()
    });
    let painted = || {
      testing::render_to_colors(12, 2)[..12]
        .iter()
        .filter(|colors| colors.1 == Some(rgb(red())))
        .count()
    };

    assert_eq!(painted(), 0);

    width.set(8);

    assert_eq!(painted(), 0, "the first frame paints where it begins");

    testing::trigger_frame(Duration::from_millis(250));

    assert_eq!(painted(), 2);

    width.set(0);

    assert_eq!(
      painted(),
      2,
      "the reversal begins where the transition stands"
    );

    testing::trigger_frame(Duration::from_millis(125));

    assert_eq!(painted(), 1, "half of the reversed quarter");

    testing::trigger_frame(Duration::from_millis(125));

    assert_eq!(painted(), 0, "back where it began, a quarter in");
  });
}

/// A width transition whose easing overshoots below zero holds the box at zero for
/// those frames, as a browser holds an interpolated width, rather than laying it out
/// at its stretched size.
#[test]
fn an_overshooting_width_transition_holds_at_zero() {
  with_platform(|| {
    let width = Signal::new(2i32);
    let _ = mount(move || {
      Column::new()
        .set_children((Container::new()
          .derive_width(move || Dimension::Length(width.get()))
          .set_height(1)
          .set_background(red())
          .add_transition(
            Transition::new(Property::Width, 1.).set_easing(Easing::CubicBezier(0.4, -2., 0.6, 1.)),
          ),))
        .into_any()
        .build()
    });
    let painted = || {
      testing::render_to_colors(12, 2)[..12]
        .iter()
        .filter(|colors| colors.1 == Some(rgb(red())))
        .count()
    };

    assert_eq!(painted(), 2);

    width.set(10);

    let mut seen = Vec::new();

    for _ in 0..10 {
      testing::trigger_frame(Duration::from_millis(100));
      seen.push(painted());
    }

    testing::trigger_frame(Duration::from_millis(100));

    assert!(
      seen.contains(&0),
      "the easing overshoots below zero: {seen:?}"
    );
    assert!(seen.iter().all(|cells| *cells <= 10), "{seen:?}");
    assert_eq!(painted(), 10, "{seen:?}");
  });
}

/// Reduced motion cuts a transition that respects it and leaves one that opted out
/// running.
#[test]
fn reduced_motion_cuts_a_respecting_transition() {
  with_platform(|| {
    let _ = mount(|| {
      Row::new()
        .set_children((
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(red().when(State::Hovered, blue()))
            .add_transition(color_transition()),
          Container::new()
            .set_width(2)
            .set_height(1)
            .set_background(red().when(State::Hovered, blue()))
            .add_transition(color_transition().set_respects_reduced_motion(false)),
        ))
        .into_any()
        .build()
    });

    testing::trigger_reduced_motion(true);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    testing::trigger_mouse_move(0, 0);

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    testing::trigger_mouse_move(2, 0);

    assert_eq!(colors_at(6, 2, 2, 0).1, Some(rgb(red())));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 2, 0).1, Some(rgb(red().mix(blue(), 0.5))));
  });
}

/// A palette flip moves a token-driven color through its transition without touching
/// the node.
#[test]
fn a_palette_flip_transitions_a_token_driven_color() {
  with_platform(|| {
    let palette = Signal::new(SimplePalette::default().set_card(red()));
    let _ = mount(move || {
      Theme::derive(move || palette.get())
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(1)
            .set_background(SimplePalette::CARD)
            .add_transition(color_transition())
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    palette.update(|palette| *palette = palette.clone().set_card(blue()));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    testing::trigger_reduced_motion(true);
    palette.update(|palette| *palette = palette.clone().set_card(red()));
    testing::trigger_frame(Duration::from_millis(1));

    assert_eq!(
      colors_at(6, 2, 0, 0).1,
      Some(rgb(red())),
      "reduced motion cuts the flip"
    );
  });
}

/// A keyframe animation samples between its frames on the clock and rests on its last
/// frame; its token leaves resolve through the theme.
#[test]
fn a_keyframe_animation_samples_its_frames() {
  with_platform(|| {
    let _ = mount(|| {
      Theme::new(SimplePalette::default().set_card(blue()))
        .set_child_builder(|| {
          Container::new()
            .set_width(4)
            .set_height(1)
            .set_background(red())
            .add_animation(
              Animation::new(1., [(0., frame(red())), (100., frame(SimplePalette::CARD))])
                .set_easing(Easing::Linear),
            )
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    let resolved = testing::style_resolutions();

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));
    assert_eq!(
      testing::style_resolutions(),
      resolved,
      "a running animation samples its resolved keyframes, resolving nothing per frame"
    );

    testing::trigger_frame(Duration::from_secs(2));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));
  });
}

/// A keyframe animation setting a color at its midpoint alone moves from the resolved
/// color and back, as CSS's neutral keyframes do, and the resolved color stands once it
/// ends.
#[test]
fn a_keyframe_missing_its_ends_moves_from_the_resolved_value() {
  with_platform(|| {
    let _ = mount(|| {
      Container::new()
        .set_width(4)
        .set_height(1)
        .set_background(red())
        .add_animation(Animation::new(1., [(50., frame(blue()))]).set_easing(Easing::Linear))
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));

    testing::trigger_frame(Duration::from_millis(250));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

    testing::trigger_frame(Duration::from_millis(250));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(blue())));

    testing::trigger_frame(Duration::from_secs(1));

    assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())));
  });
}

/// A run's keyframe animation samples its text color between its frames and rests on
/// its last frame; its token leaves resolve through the theme.
#[test]
fn a_runs_keyframe_animation_samples_its_frames() {
  with_platform(|| {
    let _ = mount(|| {
      Theme::new(SimplePalette::default().set_card(blue()))
        .set_child_builder(|| {
          Text::new("ink").set_color(red()).add_animation(
            Animation::new(
              1.,
              [
                (0., ink_frame(red())),
                (100., ink_frame(SimplePalette::CARD)),
              ],
            )
            .set_easing(Easing::Linear),
          )
        })
        .into_any()
        .build()
    });

    assert_eq!(colors_at(6, 2, 0, 0).0, Some(rgb(red())));

    let resolved = testing::style_resolutions();

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).0, Some(rgb(red().mix(blue(), 0.5))));

    testing::trigger_frame(Duration::from_millis(500));

    assert_eq!(colors_at(6, 2, 0, 0).0, Some(rgb(blue())));
    assert_eq!(
      testing::style_resolutions(),
      resolved,
      "a running animation samples its resolved keyframes, resolving nothing per frame"
    );

    testing::trigger_frame(Duration::from_secs(2));

    assert_eq!(colors_at(6, 2, 0, 0).0, Some(rgb(blue())));
  });
}

/// A closed overlay runs no motion, and its animations start over each time it opens,
/// as a browser's do once an element leaves `display: none`.
#[test]
fn an_overlays_animation_starts_over_when_it_opens() {
  with_platform(|| {
    let open = Signal::new(false);

    let _ = mount(move || {
      Column::new()
        .set_children((
          Text::new("page").into_any(),
          Overlay::popup()
            .bind_open(open)
            .set_placement(Placement::TopStart)
            .set_child(
              Container::new()
                .set_width(4)
                .set_height(1)
                .set_background(red())
                .add_animation(
                  Animation::new(1., [(0., frame(red())), (100., frame(blue()))])
                    .set_easing(Easing::Linear),
                ),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(6, 2);

    testing::trigger_frame(Duration::from_secs(5));

    let _ = testing::render_to_text(6, 2);

    assert!(
      !testing::is_frame_pending(),
      "a closed overlay runs no motion"
    );

    for _ in 0..2 {
      open.set(true);

      assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red())), "the first frame");

      testing::trigger_frame(Duration::from_millis(500));

      assert_eq!(colors_at(6, 2, 0, 0).1, Some(rgb(red().mix(blue(), 0.5))));

      open.set(false);

      let _ = testing::render_to_text(6, 2);

      testing::trigger_frame(Duration::from_secs(5));
    }
  });
}
