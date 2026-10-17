//! What crosses the seam for a control Skene's views compose: the state its parts are styled
//! under, the parts a control exposes, where its label sits, how a press drags it, the verdict
//! a handler gives an event, what a reader asks of it, what its node is, and what a context
//! menu's standard entries ask of the text they act on.

use std::rc::Rc;

use crate::{
  State,
  seam::{
    access::Checked,
    report::{KeyEvent, PointerEvent},
  },
};

/// The state the control-scoped conditions inside a control resolve against
/// ([`State::is_control_state`]), with the press its behavior holds from the keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ControlState {
  /// Whether the control is on, off or mixed.
  pub checked: Checked,
  /// Whether the control is present but not operable.
  pub disabled: bool,
  /// Whether its value is invalid.
  pub invalid: bool,
  /// Whether its popup is open.
  pub open: bool,
  /// Whether its behavior holds it pressed (`Space` held), beside the pointer's press.
  pub pressed: bool,
}

impl ControlState {
  /// Off, enabled, valid, closed and not held.
  pub const DEFAULT: Self = Self {
    checked: Checked::Off,
    disabled: false,
    invalid: false,
    open: false,
    pressed: false,
  };

  /// Whether `state` holds of the control itself (`Checked` for on, `Indeterminate` for
  /// mixed, `Pressed` where its behavior holds it); `false` for the states the realization
  /// reports.
  #[must_use]
  pub const fn holds(self, state: State) -> bool {
    match state {
      State::Checked => matches!(self.checked, Checked::On),
      State::Indeterminate => matches!(self.checked, Checked::Mixed),
      State::Disabled => self.disabled,
      State::Invalid => self.invalid,
      State::Open => self.open,
      State::Pressed => self.pressed,
      State::Hovered | State::Focused | State::FocusVisible | State::FocusWithin => false,
    }
  }
}

impl Default for ControlState {
  #[inline(always)]
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// A styling target a control exposes: a node built as a part is that part of the nearest
/// enclosing control, its control-scoped states the control's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Part {
  /// The control's own box.
  Root,
  /// The groove a value moves along: a switch's track, a slider's track.
  Track,
  /// The part of a track up to the value.
  Fill,
  /// What the reader drags or flips: a slider's thumb, a switch's knob.
  Thumb,
  /// A drawn mark: a check, a dot, a chevron.
  Mark,
  /// The box a mark sits in: a checkbox's or a radio's face.
  Face,
  /// A popup's list.
  List,
  /// An item of a list.
  Item,
  /// The control's caption.
  Label,
  /// A tooltip's bubble.
  Bubble,
}

impl Part {
  /// Whether a reader skips the part: all but a label, a list and an item.
  #[must_use]
  pub const fn is_presentational(self) -> bool {
    !matches!(self, Self::Label | Self::List | Self::Item)
  }
}

/// Where a control's label sits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LabelPlacement {
  /// First in the control's row or column.
  Leading,
  /// Last in it.
  Trailing,
  /// Before the control's box among its siblings, outside its decoration, a press on it
  /// focusing the control.
  Beside,
}

/// The axes a press drags a control along.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DragAxes {
  /// Side to side.
  Horizontal,
  /// Up and down.
  Vertical,
  /// Both ways.
  Both,
}

/// What a handler's verdict keeps from happening.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Prevent {
  /// Nothing: the platform's default runs and the event climbs on.
  #[default]
  None,
  /// The platform's default (moving focus, scrolling, editing); the ancestors still hear it.
  Platform,
  /// The climb; the platform's default still runs.
  Bubbling,
  /// Both.
  All,
}

impl Prevent {
  /// Whether the platform's default is kept from running.
  #[must_use]
  pub const fn prevents_platform(self) -> bool {
    matches!(self, Self::Platform | Self::All)
  }

  /// Whether the climb ends here.
  #[must_use]
  pub const fn stops(self) -> bool {
    matches!(self, Self::Bubbling | Self::All)
  }

  /// This verdict and `other` both: what an event that met both keeps from happening.
  #[must_use]
  pub const fn and(self, other: Self) -> Self {
    match (
      self.prevents_platform() || other.prevents_platform(),
      self.stops() || other.stops(),
    ) {
      (false, false) => Self::None,
      (true, false) => Self::Platform,
      (false, true) => Self::Bubbling,
      (true, true) => Self::All,
    }
  }
}

/// What a reader asks of a node through its accessibility layer.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AccessRequest {
  /// Activate it (press, click).
  Activate,
  /// Flip it.
  Toggle,
  /// Step it up.
  Increment,
  /// Step it down.
  Decrement,
  /// Open it.
  Expand,
  /// Close it.
  Collapse,
  /// Move focus to it.
  Focus,
  /// Set a range's value.
  SetValue(f64),
}

/// What a control's node is on a platform that draws it: a box Skene's views compose, or the
/// realization's own text view.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Element {
  /// A box: a row or a column of its label, parts and body.
  Box,
  /// The realization's own editable text view.
  Editable {
    /// The lines it shows at once; more than one makes it an editor.
    lines: u16,
  },
}

/// What a control answers, registered once on its node ([`crate::seam::tree::handle_control`]):
/// each closure is its behavior's, the control handing it the handle it acts through.
pub struct ControlDispatch {
  /// A key pressed or released while focus is on the control or inside it.
  pub key: Rc<dyn Fn(&KeyEvent) -> Prevent>,
  /// A pointer event inside the control, or captured by it during a drag.
  pub pointer: Rc<dyn Fn(&PointerEvent) -> Prevent>,
  /// A click: a primary press and its release over the control.
  pub click: Rc<dyn Fn() -> Prevent>,
  /// A reader's request; whether the behavior took it.
  pub access: Rc<dyn Fn(AccessRequest) -> bool>,
  /// Focus left the control.
  pub blur: Rc<dyn Fn()>,
  /// Whether the control is disabled now: a disabled control swallows presses, clicks, keys
  /// and a reader's requests.
  pub disabled: Rc<dyn Fn() -> bool>,
}

/// What a context menu's standard entry asks of the text it is opened on: the clipboard's
/// three, and deleting and selecting all, which stand beside them in every edit menu.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ClipboardOp {
  /// Puts the selection on the clipboard and removes it.
  Cut,
  /// Puts the selection on the clipboard.
  Copy,
  /// Replaces the selection with the clipboard's text.
  Paste,
  /// Removes the selection.
  Delete,
  /// Selects the whole text.
  SelectAll,
}

/// Whether a context menu's entry is offered where it is opened.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Availability {
  /// Not there: the menu leaves the entry out.
  Absent,
  /// There, but refused now: shown and not operable.
  Disabled,
  /// There and operable.
  Enabled,
}
