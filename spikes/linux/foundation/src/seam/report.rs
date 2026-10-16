//! What a realization reports from the platform, and the answers it gets.

use std::rc::Rc;

use crate::{
  State,
  seam::{
    content::{Content, PropValue, Verdict},
    control::{AccessRequest, Prevent},
  },
};

/// A pointer's phase.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PointerPhase {
  /// Pressed.
  Down,
  /// Moved.
  Move,
  /// Released.
  Up,
  /// Left the node.
  Leave,
  /// Cancelled by the platform (a gesture took over).
  Cancel,
}

/// A pointer event at a node, in the node's coordinates and the window's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointerEvent {
  /// The phase.
  pub phase: PointerPhase,
  /// The horizontal position, from the node's leading edge.
  pub x: f32,
  /// The vertical position, from the node's top.
  pub y: f32,
  /// The horizontal position in the window: what a part's frame is measured against.
  pub window_x: f32,
  /// The vertical position in the window.
  pub window_y: f32,
  /// The button, 0 the primary.
  pub button: u8,
}

/// A key event, named as a browser names it (`key` and `code`).
#[derive(Clone, PartialEq, Debug)]
pub struct KeyEvent {
  /// The key's value under the layout and modifiers (`"a"`, `"Enter"`).
  pub key: String,
  /// The physical key (`"KeyA"`).
  pub code: String,
  /// Whether it is pressed (else released).
  pub down: bool,
  /// Whether it repeats.
  pub repeat: bool,
  /// The modifiers held.
  pub modifiers: Modifiers,
}

/// Modifier keys held.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per key")]
pub struct Modifiers {
  /// Shift.
  pub shift: bool,
  /// Control.
  pub control: bool,
  /// Alt or Option.
  pub alt: bool,
  /// Super, Command or Windows.
  pub meta: bool,
}

/// What a realization reports at a node.
#[derive(Clone, PartialEq, Debug)]
pub enum Report {
  /// The reader changed a value to `proposed`: answered with a [`Verdict`] at once.
  Change {
    /// The value's name.
    content: Content,
    /// What the reader made it.
    proposed: PropValue,
    /// Whether the toolkit has applied it already (a toggle that flips before telling, a
    /// browser's `input`, an IME commit, a native undo): a `Replace` is then repaired, and
    /// held while an input method composes.
    applied: bool,
  },
  /// The reader edited a text entry's text: `start..end` of the text it showed, in the
  /// realization's [`crate::seam::Realization::UNIT`], replaced by `replacement`, the caret after
  /// it. Answered with a [`Verdict`] at once, as a change is.
  Edit {
    /// Where the replaced range starts.
    start: usize,
    /// Where it ends.
    end: usize,
    /// What replaces it.
    replacement: Rc<str>,
    /// Whether the toolkit has applied it already.
    applied: bool,
  },
  /// The caret or the selection of a text entry moved, in the realization's unit over the
  /// text it shows: one report per move however many toolkit notifications made it.
  Selection {
    /// Where the selection was started.
    anchor: usize,
    /// Where the caret is.
    caret: usize,
  },
  /// A click: a primary press and its release over one owner (a control, a box that acts).
  /// It does not climb.
  Click,
  /// What a reader asked of the node through its accessibility layer: answered by the
  /// control's behavior, an unanswered `Activate` being a click.
  Access(AccessRequest),
  /// A press outside an open overlay and its anchor, or the platform's own dismissal of it:
  /// heard by the overlay's dismiss handler.
  Dismiss,
  /// The reader followed a link to this destination: an acting box with one, or a link run
  /// of rich text. It climbs to the nearest follow handler; where none takes it, the
  /// realization opens it with the platform.
  Follow(Rc<str>),
  /// An input method started or stopped composing in the node: a write or a repair held for
  /// it is settled when it stops, after the commit that ends it.
  Composition(bool),
  /// Focus entered or left the node.
  Focus(bool),
  /// A state the shared half asked for changed ([`Interest::states`]).
  State {
    /// The state.
    state: State,
    /// Whether it holds now.
    holds: bool,
  },
  /// A pointer event at the deepest node under the pointer, where asked
  /// ([`Interest::pointer`]), or at the control a drag captured it for. It climbs the scene
  /// and stops at the first control or acting box: a view that handles an interaction owns
  /// it; a disabled control swallows it.
  Pointer(PointerEvent),
  /// The node's size changed, where asked ([`Interest::size`]): what it was laid out at.
  Size(Size),
  /// A key event at the focused node, before the platform's default (moving focus,
  /// scrolling, a text entry's editing). It climbs every ancestor, each control's behavior
  /// answering first, until a verdict stops it.
  Key(KeyEvent),
}

/// A node's size as laid out.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Size {
  /// The width, in pixels.
  pub width: f32,
  /// The height, in pixels.
  pub height: f32,
  /// Device pixels per pixel.
  pub scale: f64,
}

/// What a report is answered with.
#[derive(Clone, PartialEq, Debug)]
pub enum Answer {
  /// A change's verdict.
  Verdict(Verdict),
  /// An event's fate: what the handlers it met keep from happening, folded, so the
  /// realization runs the platform's default unless one prevented it.
  Event {
    /// What was prevented.
    prevented: Prevent,
  },
  /// Nothing to answer.
  None,
}

/// What a realization reports at a node beyond changes and activation: the shared half asks
/// only where a record or a handler needs it.
///
/// [`crate::seam::Realization::watch`] receives a node's whole interest each time it grows; a
/// realization installs what it lacks and nothing twice.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Interest {
  /// The states to report.
  pub states: crate::StateSet,
  /// Whether to report pointer events.
  pub pointer: bool,
  /// Whether to report key events.
  pub keys: bool,
  /// Whether to report focus.
  pub focus: bool,
  /// Whether to report the node's size as laid out.
  pub size: bool,
}
