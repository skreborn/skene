//! A ready-made radio group and its radios, in the Simple design language.

use skene_foundation::{Axis, ItemAlignment};
use skene_tui::{Choice, ControlBase, Part, Radio, RadioGroup, Stack};

use crate::{SimpleGlyphs, glyph_mark};

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
/// use skene_simple_tui::prelude::*;
/// use skene_tui::prelude::*;
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
///   .set_child(Column::new().set_spacing(1).set_children((
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

/// A ready-made radio, a preset of [`Radio`]: a glyph mark beside its caption.
///
/// Its mark is the glyphs of [`SimpleGlyphs`], `(x)` while chosen and `( )` otherwise
/// unless a theme redefines them, inked in the foreground and the ring while focus shows.
/// [`new`](Self::new) returns the radio, so every member of a radio applies;
/// [`styled`](Self::styled) dresses a radio built otherwise, and [`mark`](Self::mark) is
/// the mark it shows, its [`Part::Mark`](skene_tui::Part::Mark), which follows
/// whether its group has chosen it. Outside a group it draws, is never chosen and takes
/// focus like any control. Its colors are [`SimplePalette`](crate::SimplePalette)'s
/// tokens.
///
/// ```
/// use skene_simple_tui::prelude::*;
/// use skene_tui::prelude::*;
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

  /// `radio` showing the Simple mark.
  pub fn styled(radio: Radio) -> Radio {
    radio.set_part(Part::Mark, Self::mark())
  }

  /// The mark: the chosen glyph while the group has chosen the radio and the unchosen one
  /// otherwise, in one place.
  pub fn mark() -> Stack {
    glyph_mark(SimpleGlyphs::radio)
  }
}
