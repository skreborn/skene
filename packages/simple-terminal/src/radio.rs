//! A ready-made radio group and its members, in the Simple design language.

use skene_foundation::{Choice, View, Wraps};
use skene_simple::{SimpleRadioBase, SimpleRadioGroupBase};
use skene_terminal::Active;

/// A ready-made group of radio buttons: the members inside it are one tab stop, the
/// arrow keys move between them, and exactly one is chosen.
///
/// Members are [`SimpleRadio`]s at any depth inside the child, tagged with values of the
/// app's own type `T`, so one may be wrapped or interleaved with anything. The chosen
/// value flows both ways; a group that starts with nothing chosen holds an `Option`, its
/// members tagged `Some(..)`. Without the view
/// [`set_child`](SimpleRadioGroupBase::set_child) supplies, it builds an empty group.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let sizes =
///   SimpleRadioGroup::new()
///     .bind_chosen(size)
///     .set_child(Column::new().set_spacing(1).set_children((
///       SimpleRadio::new(Size::Small).set_label("Small"),
///       SimpleRadio::new(Size::Medium).set_label("Medium"),
///     )));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleRadioGroup<T: Choice>(skene_simple::SimpleRadioGroup<Active, T>);

impl<T: Choice> Default for SimpleRadioGroup<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Choice> SimpleRadioGroupBase<Active, T> for SimpleRadioGroup<T> {}

/// A ready-made radio button: a ring on the card surface whose dot appears while
/// chosen and which rings while focused, beside its caption.
///
/// The ring is the control's [`Part::Mark`](skene_terminal::Part::Mark), so it follows the
/// control's state. Outside a [`SimpleRadioGroup`] it draws, is never chosen and
/// takes focus like any control. For a radio drawn some other way, compose
/// [`BasicRadio`](skene_terminal::BasicRadio) directly.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::prelude::*;
///
/// let small = SimpleRadio::new(1).set_label("Small");
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleRadio(skene_simple::SimpleRadio<Active>);

impl SimpleRadioBase<Active> for SimpleRadio {}
