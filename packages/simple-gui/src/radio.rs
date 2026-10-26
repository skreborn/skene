//! A ready-made radio group and its radios, in the Simple design language.

use skene_foundation::{Alignment, Axis, Dimension, ItemAlignment};
use skene_gui::{
  Choice,
  Color,
  Container,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  HasChild,
  Inset,
  Part,
  Radio,
  RadioGroup,
  State,
  When,
};

use crate::{SimplePalette, shape};

/// A ready-made radio group, a preset of [`RadioGroup`]: its caption beside its radios,
/// centered on the row they share.
///
/// [`new`](Self::new) returns the group, so every member of a radio group applies: its
/// radios, [`SimpleRadio`]s or any [`Radio`], tagged with values of the app's own type
/// `T`, come from its `set_children` or a body `set_child` holds at any depth, and the
/// chosen value flows both ways; a group that starts with nothing chosen holds an
/// `Option`, its radios tagged `Some(..)`. [`styled`](Self::styled) dresses a group built
/// otherwise.
///
/// ```
/// use skene_gui::prelude::*;
/// use skene_simple_gui::prelude::*;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let sizes = SimpleRadioGroup::new()
///   .set_label("Size")
///   .bind_chosen(size)
///   .set_child(Column::new().set_spacing(8).set_children((
///     SimpleRadio::new(Size::Small).set_label("Small"),
///     SimpleRadio::new(Size::Medium).set_label("Medium"),
///   )));
/// ```
#[derive(Debug)]
pub enum SimpleRadioGroup {}

impl SimpleRadioGroup {
  /// A group with nothing chosen and no radios.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new<T: Choice>() -> RadioGroup<T> {
    Self::styled(RadioGroup::new())
  }

  /// `group` with its caption beside its radios, centered across the row.
  pub fn styled<T: Choice>(group: RadioGroup<T>) -> RadioGroup<T> {
    group
      .set_axis(Axis::Horizontal)
      .set_item_alignment(ItemAlignment::Center)
  }
}

/// A ready-made radio, a preset of [`Radio`]: a drawn ring beside its caption.
///
/// Its ring sits on the card surface, its dot filling with the primary surface while
/// chosen, and rings while focus shows. [`new`](Self::new) returns the radio, so every
/// member of a radio applies; [`styled`](Self::styled) dresses a radio built otherwise,
/// and [`mark`](Self::mark) is the ring it shows, its
/// [`Part::Mark`](skene_gui::Part::Mark), which follows whether its group has chosen it.
/// Outside a group it draws, is never chosen and takes focus like any control. Its colors
/// are [`SimplePalette`]'s tokens.
///
/// ```
/// use skene_gui::prelude::*;
/// use skene_simple_gui::prelude::*;
///
/// let small = SimpleRadio::new(1).set_label("Small");
/// ```
#[derive(Debug)]
pub enum SimpleRadio {}

impl SimpleRadio {
  /// A radio tagged with `value`, what its group is asked to choose when it is chosen.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(value: impl Choice) -> Radio {
    Self::styled(Radio::new(value))
  }

  /// `radio` showing the Simple ring.
  pub fn styled(radio: Radio) -> Radio {
    radio.set_part(Part::Mark, Self::mark())
  }

  /// The ring: on the card surface, holding a dot that fills with the primary surface
  /// while the radio is chosen, ringing while focus shows.
  pub fn mark() -> Container {
    let shape = shape();
    let dot = Container::new()
      .set_width(Dimension::Length(shape.dot_size))
      .set_height(Dimension::Length(shape.dot_size))
      .set_corner_radius(shape.dot_size / 2.)
      .set_background(Color::TRANSPARENT.when(State::Checked, SimplePalette::PRIMARY_SURFACE));

    Container::new()
      .set_width(Dimension::Length(shape.mark_size))
      .set_height(Dimension::Length(shape.mark_size))
      .set_background(shape.mark_surface)
      .set_border_width(Inset::all(shape.mark_ring))
      .set_border_color(
        SimplePalette::BORDER
          .when(State::Checked, SimplePalette::PRIMARY_SURFACE)
          .when(State::FocusVisible, SimplePalette::RING),
      )
      .set_corner_radius(shape.mark_size / 2.)
      .set_alignment(Alignment::CENTER)
      .set_child(dot)
  }
}
