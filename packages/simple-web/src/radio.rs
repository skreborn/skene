//! A ready-made radio group and its members, in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::{SimpleRadioBase, SimpleRadioGroupBase};
use skene_web::Active;

/// A ready-made group of radio buttons: the members inside it are one tab stop, the
/// arrow keys move between them, and exactly one is chosen.
///
/// Members are [`SimpleRadio`]s at any depth inside the child, so one may be wrapped
/// or interleaved with anything. The chosen value flows both ways. Without the view
/// [`set_child`](Self::set_child) supplies, it builds an empty group.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
///
/// let size = Signal::new(Some(String::from("m")));
/// let sizes = SimpleRadioGroup::new().bind_chosen(size).set_child(
///   Column::new().set_spacing(12.).set_children((
///     SimpleRadio::new("s").set_label("Small"),
///     SimpleRadio::new("m").set_label("Medium"),
///   )),
/// );
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleRadioGroup(skene_simple::SimpleRadioGroup<Active>);

impl Default for SimpleRadioGroup {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl SimpleRadioGroupBase<Active> for SimpleRadioGroup {}

/// A ready-made radio button: a ring on the card surface whose dot appears while
/// chosen and which rings while focused, beside its caption.
///
/// The ring is the control's [`Part::Mark`](skene_web::Part::Mark), so it follows the
/// control's state. Outside a [`SimpleRadioGroup`] it draws, is never chosen and
/// takes focus like any control. For a radio drawn some other way, compose
/// [`BasicRadio`](skene_web::BasicRadio) directly.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
///
/// let small = SimpleRadio::new("s").set_label("Small");
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleRadio(skene_simple::SimpleRadio<Active>);

impl SimpleRadioBase<Active> for SimpleRadio {}
