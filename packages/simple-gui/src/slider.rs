//! A ready-made slider in the Simple design language.

use std::rc::Rc;

use skene_foundation::{
  Alignment,
  Dimension,
  FlexChild,
  Flexibility,
  ItemAlignment,
  Scalar,
  SliderContext,
};
use skene_gui::{
  AnyView,
  Container,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  Flexible,
  FlexibleBase,
  HasChild,
  HasChildren,
  Inset,
  IntoChildOf,
  NodeHandle,
  NodeId,
  Part,
  Pinned,
  PinnedBase,
  Scene,
  Slider,
  SliderBase,
  Stack,
  StackBase,
  State,
  View,
  reactive::{untrack, use_context},
};

use crate::{SimplePalette, as_part, shape};

/// A ready-made slider, a preset of [`Slider`]: a track on the secondary surface, the
/// run filled to the value in the primary surface, and a thumb that rings while focused.
///
/// [`new`](Self::new) returns the slider, full width, so every member of a slider
/// applies, its value flowing both ways; [`styled`](Self::styled) dresses a slider
/// built otherwise from the pieces [`track`](Self::track) and [`thumb`](Self::thumb).
/// The track is the control's [`Part::Track`](skene_gui::Part::Track), holding the
/// fill, its [`Part::Fill`](skene_gui::Part::Fill), and the thumb, its
/// [`Part::Thumb`](skene_gui::Part::Thumb); the fill runs to the thumb's leading edge
/// and the thumb travels the track less its own size, so both stay inside the track at
/// either end. Nothing visible names it unless its `set_label` sets a caption, so its
/// `set_accessible_name` names it otherwise. Its colors are [`SimplePalette`]'s tokens,
/// inked in the palette's default unless a [`Theme`](skene_gui::Theme) of the palette
/// above redefines them.
///
/// ```
/// use skene_gui::{behavior::SliderRange, prelude::*};
/// use skene_simple_gui::prelude::*;
///
/// let volume = Signal::new(50.);
/// let slider = SimpleSlider::new()
///   .set_accessible_name("Volume")
///   .set_range(SliderRange::new(0., 100., 5.))
///   .bind_value(volume);
/// ```
#[derive(Debug)]
pub enum SimpleSlider {}

impl SimpleSlider {
  /// A slider over `0..=100` stepping by `1`, holding its minimum.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new() -> Slider {
    Self::styled(Slider::new())
  }

  /// `slider` full width on the Simple [`track`](Self::track), the pointer measured
  /// against the Simple [`thumb`](Self::thumb).
  pub fn styled(slider: Slider) -> Slider {
    slider
      .set_width(Dimension::Fraction(1.))
      .set_thumb_extent(shape().thumb_size)
      .set_part(Part::Track, Self::track())
  }

  /// The track: a rounded rail on the secondary surface beneath the fill in the primary
  /// surface, its [`Part::Fill`](skene_gui::Part::Fill), and the lane the
  /// [`thumb`](Self::thumb) travels, its [`Part::Thumb`](skene_gui::Part::Thumb), both
  /// placed at the fraction the enclosing slider's [`SliderContext`] reads. It takes
  /// what the slider's caption leaves.
  pub fn track() -> AnyView {
    IntoChildOf::<FlexChild>::into_child(
      Flexible::new()
        .set_flexibility(Flexibility::new(0, 1., 1.))
        .set_child(AnyView::new(Self::build_track)),
    )
  }

  /// The thumb: a disc in the shape's thumb surface, its border in the background color
  /// that becomes the ring while focus shows.
  pub fn thumb() -> Container {
    let shape = shape();

    Container::new()
      .set_width(Dimension::Length(shape.thumb_size))
      .set_height(Dimension::Length(shape.thumb_size))
      .set_corner_radius(shape.thumb_size.to_f32() / 2.)
      .set_background(
        shape
          .thumb_surface
          .when(State::FocusVisible, shape.thumb_focus),
      )
      .set_border_width(Inset::all(shape.thumb_ring))
      .set_border_color(SimplePalette::BACKGROUND.when(State::FocusVisible, SimplePalette::RING))
  }

  /// Builds the track over the fraction the enclosing slider shares, the minimum's
  /// where none does, recording the thumb as its part once the stack has built it.
  fn build_track() -> NodeId {
    let shape = shape();
    let fraction: Rc<dyn Fn() -> f32> = match use_context::<SliderContext>() {
      Some(slider) => slider.reads_fraction(),
      None => Rc::new(|| 0.),
    };
    let filled = Rc::clone(&fraction);
    let rail = || {
      Container::new()
        .set_height(Dimension::Length(shape.track_height))
        .set_corner_radius(shape.track_height.to_f32() / 2.)
    };
    // Each lane spares the thumb's width at its end, so the fill's fraction and the
    // thumb's travel both span the track less the thumb.
    let lane = || {
      Container::new()
        .set_width(Dimension::Fraction(1.))
        .set_height(Dimension::Length(shape.thumb_size))
        .set_padding(Inset {
          top: 0.,
          end: shape.thumb_size,
          bottom: 0.,
          start: 0.,
        })
        .set_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
    };
    let thumb = NodeHandle::new();
    let node = Container::new()
      .set_height(Dimension::Length(shape.thumb_size))
      .set_child(
        Stack::new()
          .set_item_alignment(Alignment::new(ItemAlignment::Start, ItemAlignment::Center))
          .set_children((
            rail()
              .set_width(Dimension::Fraction(1.))
              .set_background(SimplePalette::SECONDARY_SURFACE),
            lane().set_child(as_part(
              Part::Fill,
              rail()
                .derive_width(move || Dimension::Fraction(filled()))
                .set_background(SimplePalette::PRIMARY_SURFACE),
            )),
            lane()
              .set_alignment(Alignment::new(
                ItemAlignment::Stretch,
                ItemAlignment::Stretch,
              ))
              .set_child(
                Stack::new().add_child(
                  Pinned::new()
                    .derive_edges(move || Inset {
                      top: Dimension::Length(0.),
                      end: Dimension::Auto,
                      bottom: Dimension::Auto,
                      start: Dimension::Fraction(fraction()),
                    })
                    .set_child(Self::thumb().bind_handle(thumb)),
                ),
              ),
          )),
      )
      .build();

    if let Some(thumb) = untrack(|| thumb.node()) {
      Scene::with(|scene| scene.set_part(thumb, Part::Thumb));
    }

    node
  }
}
