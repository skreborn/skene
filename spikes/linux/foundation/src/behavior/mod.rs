//! What a control does, written once for every platform that draws its controls: the keys each
//! kind of control answers (its keyboard table), what a press, a drag, a click and a reader's
//! action do to it, and what it reports to a reader.
//!
//! A behavior is a value built from the control's state as it is shown now ([`Toggle`] from
//! whether it is on, [`Slider`] from its range), asked about one event, and answered with a
//! [`Response`]: whether the control consumes the event, whether it shows pressed from the
//! keyboard, and the act it reports (a proposed value, an activation, opening its list). It
//! never changes the control itself: the realization reports the act and shows the truth the
//! app answers with, as every controlled value is shown.
//!
//! - [`Activate`]: a command (a button): `Enter` on every press, `Space` on its release.
//! - [`Toggle`]: a switch, a checkbox or a radio: `Space` on its release, a click flips it.
//! - [`Slider`]: a value on a range: the arrows step it, the page keys by a tenth, `Home` and
//!   `End` to its ends, a press jumps to where it lands and a drag follows the pointer.
//! - [`Collection`]: members of a group, one chosen (a radio group, a list): the arrows move
//!   to the next enabled member, wrapping where the collection wraps.
//! - [`Popup`]: a closed face that opens a list of options (a picker): the face's keys and the
//!   open list's, the list a [`Collection`], with [`TypeAhead`].
//! - [`TextEntry`]: a text entry: the editing core's key table
//!   ([`crate::text::editing::Binding`]), `Enter` activating an entry of one line.
//!
//! The keys follow Chromium's form controls where a platform has no native behavior of its
//! own (Linux and Windows draw every control).

mod activate;
mod collection;
mod popup;
mod slider;
mod text_entry;
mod toggle;
mod type_ahead;

#[cfg(test)]
mod tests;

pub use self::{
  activate::Activate,
  collection::{Collection, Orientation},
  popup::{Popup, PopupAct},
  slider::Slider,
  text_entry::{EntryAct, TextEntry},
  toggle::{Toggle, ToggleKind},
  type_ahead::TypeAhead,
};
use crate::seam::{
  access::{AccessFacts, Action, Role},
  report::KeyEvent,
};

/// What a behavior answers an event with.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct Response<A> {
  /// Whether the control consumes the event: it goes to no ancestor, and the platform's own
  /// default (moving focus, scrolling) does not run.
  pub consumed: bool,
  /// Whether the control now shows pressed from the keyboard (`Space` held), where that
  /// changed.
  pub pressed: Option<bool>,
  /// What the control reports.
  pub act: Option<A>,
}

impl<A> Response<A> {
  /// The event is not the control's.
  pub const IGNORED: Self = Self {
    consumed: false,
    pressed: None,
    act: None,
  };

  /// The control consumes the event and does nothing more.
  pub const CONSUMED: Self = Self {
    consumed: true,
    pressed: None,
    act: None,
  };

  /// The control consumes the event and reports `act`.
  #[inline(always)]
  pub const fn act(act: A) -> Self {
    Self {
      consumed: true,
      pressed: None,
      act: Some(act),
    }
  }

  /// The control consumes the event and reports `act` where there is one.
  #[inline(always)]
  pub const fn maybe(act: Option<A>) -> Self {
    Self {
      consumed: true,
      pressed: None,
      act,
    }
  }

  /// This response, the control shown pressed or not.
  #[inline(always)]
  pub const fn with_pressed(mut self, pressed: bool) -> Self {
    self.pressed = Some(pressed);

    self
  }

  /// This response with its act mapped.
  pub fn map<B>(self, map: impl FnOnce(A) -> B) -> Response<B> {
    Response {
      consumed: self.consumed,
      pressed: self.pressed,
      act: self.act.map(map),
    }
  }
}

/// A control's behavior: its keyboard table, what a click and a reader's action do to it, and
/// what it reports to a reader.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a control's behavior",
  note = "a behavior answers a control's keys, clicks and a reader's actions with the act the control reports"
)]
pub trait Behavior {
  /// What the control reports.
  type Act;

  /// What a reader takes the control for.
  fn role(&self) -> Role;

  /// A key pressed (or released) while the control holds focus.
  fn key(&self, event: &KeyEvent) -> Response<Self::Act>;

  /// A click: a primary press and its release over the control.
  fn click(&self) -> Option<Self::Act> {
    None
  }

  /// A reader's action on the control.
  fn action(&self, _action: Action) -> Option<Self::Act> {
    None
  }

  /// The actions the control offers a reader.
  fn actions(&self) -> Vec<Action> {
    Vec::new()
  }

  /// What the control tells a reader of its state: its check, its value, its expansion.
  fn facts(&self) -> AccessFacts {
    AccessFacts::default()
  }
}

/// Whether `event` has no modifier but `Shift`.
#[inline(always)]
fn plain(event: &KeyEvent) -> bool {
  !event.modifiers.control && !event.modifiers.alt && !event.modifiers.meta
}
