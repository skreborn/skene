//! The native door: a library's toolkit widget inside a Skene tree.
//!
//! A library's door is platform code: a `platform::macos` module builds an `NSDatePicker`,
//! adopts it ([`crate::seam::tree::adopt`]) and, where it holds a value, hands a
//! [`ControlSink`] of its own and makes the value controlled ([`crate::seam::tree::control`]
//! with a [`crate::seam::content::ContentKey::door`] key), so the controlled contract (echo
//! rule, equal-skip, veto, composition hold) is the same as a built-in's. The engine measures
//! the adopted widget through the toolkit's protocol with the toolkit's minimum as its floor,
//! and neutralizes its own alignment and margins, so it lays out as a built-in leaf does.
//!
//! The other way round needs nothing of its own: a realized Skene subtree is
//! [`crate::seam::Realization::widget`] of its root, and a native container lays it out as any
//! widget.

use crate::seam::content::{Content, PropValue};

/// The platform side of a library's controlled widget: read and write its values by name,
/// reporting nothing. The library reports what the reader did through
/// [`crate::seam::tree::report`], as a realization does.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not read and write a native widget's values",
  label = "a door's sink here",
  note = "a sink implements `ControlSink`: `read` and `write` the widget's values by the door's names"
)]
pub trait ControlSink {
  /// The value it shows now.
  fn read(&self, content: Content) -> Option<PropValue>;

  /// Shows `value`.
  fn write(&self, content: Content, value: &PropValue);

  /// Whether an input method composes in it.
  fn composing(&self) -> bool {
    false
  }
}
