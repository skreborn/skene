//! The native door: a compositor platform's own view inside a Skene tree.
//!
//! A library's door is platform code: it hands the platform's own view
//! ([`Realization::Native`](super::Realization::Native)) to the scene
//! ([`Scene::adopt`](super::Scene::adopt)), which has the platform realize it as an
//! ordinary node ([`Realization::adopt`](super::Realization::adopt)), and, where the
//! view holds a value, registers a [`ControlSink`] of its own on the node
//! ([`Scene::set_sink`](super::Scene::set_sink)) and controls the value by a key of its
//! own ([`ContentKey::door`](super::ContentKey::door)) with a
//! [`Controller`](super::Controller), so the controlled contract (the echo rule, the
//! equal-write skip, the refusal's repair and the composition hold) is a built-in's.
//! The library reports what the reader did through [`report`](super::report), with
//! whether the view shows it already, as a realization does.
//!
//! A platform whose door adopts nothing names an uninhabited type of its own as its
//! native view, so its door cannot be reached.

use std::{any::Any, fmt::Debug};

use super::{Content, ContentType, ContentValue};

/// A door content's value, erased: what a [`ContentKey::door`](super::ContentKey::door) key erases
/// its value to, and owns it again from.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a door content's value",
  label = "a door content holds a value of a content type",
  note = "implement `ContentType` for the value a door's key holds"
)]
pub trait DoorValue: Debug {
  /// The value, to downcast to the type it holds.
  fn as_any(&self) -> &dyn Any;

  /// Whether `other` holds a value of the same type, equal to this one.
  fn is_equal(&self, other: &dyn DoorValue) -> bool;
}

impl<T: ContentType> DoorValue for T {
  #[inline(always)]
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn is_equal(&self, other: &dyn DoorValue) -> bool {
    other
      .as_any()
      .downcast_ref::<T>()
      .is_some_and(|other| self == other)
  }
}

impl PartialEq for dyn DoorValue + '_ {
  #[inline(always)]
  fn eq(&self, other: &Self) -> bool {
    self.is_equal(other)
  }
}

/// The platform side of a view adopted through the native door: it reads and writes the
/// view's values by the door's names, reporting nothing.
///
/// A library writes and owns each value through its door's key
/// ([`ContentKey::erase`](super::ContentKey::erase), [`ContentKey::own`](super::ContentKey::own)),
/// and reports what the reader changes through [`report`](super::report), as a realization does.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not read and write a native view's values",
  label = "a door's sink here",
  note = "a sink implements `ControlSink`: `read` and `write` the view's values by the door's names"
)]
pub trait ControlSink {
  /// Runs `read` with what the view shows of `content` right now, where it holds the
  /// content; a content it does not hold runs nothing.
  fn read(&self, content: Content, read: &mut dyn FnMut(ContentValue<'_>));

  /// Shows `value` as the view's `content`, whole, reporting nothing.
  fn write(&self, content: Content, value: ContentValue<'_>);

  /// Whether what the view shows is the reader's until it is reported, as a text while
  /// an input method composes in it: a write waits, and so does a repair, until the
  /// report that ends it. `false`, the default, for a view where nothing composes.
  #[inline(always)]
  fn composing(&self) -> bool {
    false
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::DoorValue;
  use crate::spine::{CheckState, Content, ContentKey, ContentValue};

  /// A door's key erases its value into a door content and owns it again, an erased
  /// value equal to another of its type and value alone.
  #[test]
  fn a_door_key_erases_and_owns_its_value() {
    let key = ContentKey::<Rc<str>>::door("color");
    let color: Rc<str> = Rc::from("#ff0000");

    assert_eq!(key.content(), Content::Door("color"));
    assert_eq!(key.own(key.erase(&color)).as_deref(), Some("#ff0000"));
    assert_eq!(key.own(ContentValue::Text("#ff0000")), None);
    assert_eq!(
      ContentKey::<CheckState>::door("color").own(key.erase(&color)),
      None
    );

    let same: Rc<str> = Rc::from("#ff0000");
    let other: Rc<str> = Rc::from("#00ff00");

    assert_eq!(key.erase(&color), key.erase(&same));
    assert_ne!(key.erase(&color), key.erase(&other));
    assert!(!(&CheckState::On as &dyn DoorValue).is_equal(&color));
  }
}
