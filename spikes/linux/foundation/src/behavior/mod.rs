//! What a control's interaction is, written once for every platform that draws its controls.
//!
//! A control is a node Skene's views compose (`Control<B>` in each facade), and its behavior
//! ([`Behavior`]) says what the interaction is: its role, the events it acts on and the verdict
//! it gives each ([`Prevent`]), what it tells a reader, whether it takes focus, and the state
//! its parts are styled under ([`ControlState`]). A behavior acts through the
//! [`ControlHandle`] its control hands it, and it never changes a value itself: a controlled
//! value ([`Controlled`]) is read through what its user derives and changed only by asking
//! its user's handler, so a change the handler refuses never shows.
//!
//! The key tables every platform shares are pure values, public so a third party's control
//! answers the same keys as a built-in's ([`ActivateKeys`], [`ToggleKeys`], [`SliderKeys`],
//! [`CollectionKeys`], [`PopupKeys`], [`EntryKeys`]). A table is built from the control's
//! state as it shows now, asked about one event, and answers with a [`Response`].
//!
//! The shipped behaviors:
//!
//! - [`Activate`]: a command (a button): `Enter` on every press, `Space` on its release.
//! - [`Toggle`]: a switch or a checkbox: `Space` on its release, a click flips it.
//! - [`Slider`]: a value on a range: the arrows step it, the page keys by a tenth, `Home` and
//!   `End` to its ends, a press jumps to where it lands and a drag follows the pointer.
//! - [`Collection`] and its [`Member`]s: a group whose members each carry a value of the app's
//!   own type ([`Choice`]), what it holds chosen ([`Held`]: one value, or a list box's
//!   [`Chosen`]): the arrows move to the next enabled member in its options' order
//!   ([`Options`]), wrapping where the policy wraps.
//! - [`Popup`]: a closed face that opens a list of options (a picker), with [`TypeAhead`].
//! - [`TextEntry`]: a text entry: the realization's own text view, `Enter` submitting an
//!   entry of one line.
//!
//! The keys follow Chromium's form controls where a platform has no native behavior of its
//! own (Linux and Windows draw every control).

mod activate;
mod collection;
mod controlled;
mod handle;
mod popup;
mod slider;
mod text_entry;
mod toggle;
mod type_ahead;

#[cfg(test)]
mod tests;

pub use self::{
  activate::{Activate, ActivateKeys},
  collection::{
    AnyChoice, AnyChosen, Choice, Chosen, Collection, CollectionKeys, CollectionPolicy, Held,
    Listbox, ListboxOption, ListboxPolicy, Member, Menu, MenuOption, MenuPolicy, One, Options,
    Orientation, Radio, RadioGroup, RadioPolicy, Roster,
  },
  controlled::Controlled,
  handle::{ContentHandler, ContentReader, ControlHandle, ControlOps},
  popup::{Popup, PopupAct, PopupKeys},
  slider::{Slider, SliderKeys},
  text_entry::{EntryAct, EntryKeys, TextEntry},
  toggle::{Toggle, ToggleKeys, ToggleKind},
  type_ahead::TypeAhead,
};
use crate::seam::{
  access::{AccessFacts, Role},
  control::{AccessRequest, ControlState, DragAxes, Element, LabelPlacement, Prevent},
  report::{KeyEvent, PointerEvent},
};

/// What a key table answers an event with.
#[derive(Clone, PartialEq, Debug)]
#[must_use]
pub struct Response<A> {
  /// Whether the control consumes the event: it goes to no ancestor, and the platform's own
  /// default (moving focus, scrolling) does not run.
  pub consumed: bool,
  /// Whether the control now shows pressed from the keyboard (`Space` held), where that
  /// changed.
  pub pressed: Option<bool>,
  /// What the control does.
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

  /// The control consumes the event and does `act`.
  #[inline(always)]
  pub const fn act(act: A) -> Self {
    Self {
      consumed: true,
      pressed: None,
      act: Some(act),
    }
  }

  /// The control consumes the event and does `act` where there is one.
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

  /// The verdict a behavior gives for this response: everything prevented where the control
  /// consumes the event, nothing otherwise.
  #[must_use]
  pub const fn verdict(&self) -> Prevent {
    if self.consumed {
      Prevent::All
    } else {
      Prevent::None
    }
  }
}

/// What a control's interaction is: its role, the events it acts on and the verdict it gives
/// each, what it tells a reader, whether it takes focus, and the state its parts are styled
/// under.
///
/// Every method has a default but the role, so a behavior answers only what it acts on. The
/// handlers run untracked; [`Behavior::access`], [`Behavior::focusable`] and
/// [`Behavior::state`] run under an effect, kept current by the signals they read.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a control's behavior",
  note = "a behavior answers a control's keys, pointer, clicks and a reader's requests, and reports its role, its facts and its state"
)]
pub trait Behavior: 'static {
  /// What a reader takes the control for.
  fn role(&self) -> Role;

  /// What the control's node is: a box, or the realization's text view.
  fn element(&self) -> Element {
    Element::Box
  }

  /// The axes a press drags it along; the platform keeps the pointer with it until the
  /// release.
  fn drag_axes(&self) -> Option<DragAxes> {
    None
  }

  /// Where its label sits unless its user says otherwise.
  fn label_placement(&self) -> LabelPlacement {
    LabelPlacement::Trailing
  }

  /// Runs once its node exists, before its parts build: binds the contents it keeps.
  fn attach(&self, _control: &ControlHandle) {}

  /// Runs once its parts and body are in place: finds what it measures against.
  fn mounted(&self, _control: &ControlHandle) {}

  /// A click: a primary press and its release over the control.
  fn on_click(&self, _control: &ControlHandle) -> Prevent {
    Prevent::None
  }

  /// A key pressed or released while focus is on the control or inside it (it climbed from a
  /// member).
  fn on_key(&self, _event: &KeyEvent, _control: &ControlHandle) -> Prevent {
    Prevent::None
  }

  /// A pointer event inside the control, or captured by it during a drag.
  fn on_pointer(&self, _event: &PointerEvent, _control: &ControlHandle) -> Prevent {
    Prevent::None
  }

  /// Focus left the control.
  fn on_blur(&self, _control: &ControlHandle) {}

  /// A reader's request; whether the behavior took it. An `Activate` it does not take is a
  /// click.
  fn on_access(&self, _request: AccessRequest, _control: &ControlHandle) -> bool {
    false
  }

  /// What it tells a reader beside its role.
  fn access(&self) -> AccessFacts {
    AccessFacts::default()
  }

  /// Whether it takes focus while enabled (a radio only while it is its group's stop).
  fn focusable(&self) -> bool {
    true
  }

  /// The state its parts' control-scoped conditions resolve against.
  fn state(&self) -> ControlState {
    ControlState::DEFAULT
  }
}

/// Whether `event` has no modifier but `Shift`.
#[inline(always)]
const fn plain(event: &KeyEvent) -> bool {
  !event.modifiers.control && !event.modifiers.alt && !event.modifiers.meta
}
