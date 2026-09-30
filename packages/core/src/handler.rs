//! The handlers a node carries.

use std::{fmt::Debug, ops::Deref, rc::Rc};

use skene_reactive::Owner;

use crate::{
  ClickHandler,
  DragSource,
  DropHandler,
  EventKind,
  KeyHandler,
  PointerHandler,
  ScrollEvent,
  TextSelection,
  WheelHandler,
};

/// A handler taking nothing: pointer enter and leave, focus and blur, dismiss.
pub type PlainHandler = dyn Fn();
/// A scroll handler: the container's new offsets.
pub type ScrollHandler = dyn Fn(&ScrollEvent);
/// A change handler: a control's new textual value.
pub type ChangeHandler = dyn Fn(&str);
/// A toggle handler: a control's new checked state.
pub type ToggleHandler = dyn Fn(bool);
/// A selection handler: a text entry's new selection.
pub type SelectionHandler = dyn Fn(TextSelection);

/// A handler for one kind of event, as a node carries it.
///
/// Every handler on a node runs when its event reaches the node, in registration
/// order; bubbling is between nodes.
#[derive(Clone)]
pub enum Handler {
  /// Runs on a click, after the node's acting clicks.
  Click(Rc<ClickHandler>),
  /// A control's own response to a click: run before the node's other click
  /// handlers, and skipped by an observed dispatch, where the platform acted.
  ActingClick(Rc<ClickHandler>),
  /// Runs on the second click of a double click.
  DoubleClick(Rc<ClickHandler>),
  /// Runs on a key press.
  KeyDown(Rc<KeyHandler>),
  /// Runs on a key release.
  KeyUp(Rc<KeyHandler>),
  /// Runs on a pointer press.
  PointerDown(Rc<PointerHandler>),
  /// Runs on a pointer release.
  PointerUp(Rc<PointerHandler>),
  /// Runs on a pointer move.
  PointerMove(Rc<PointerHandler>),
  /// Runs when the platform takes a pressed pointer for itself.
  PointerCancel(Rc<PointerHandler>),
  /// Runs when the pointer enters the node.
  PointerEnter(Rc<PlainHandler>),
  /// Runs when the pointer leaves the node.
  PointerLeave(Rc<PlainHandler>),
  /// Runs when focus enters the node's subtree.
  Focus(Rc<PlainHandler>),
  /// Runs when focus leaves the node's subtree.
  Blur(Rc<PlainHandler>),
  /// Runs on a wheel's turn, before the platform scrolls by it.
  Wheel(Rc<WheelHandler>),
  /// Runs with the node's new scroll offsets.
  Scroll(Rc<ScrollHandler>),
  /// Runs with a control's new textual value.
  Change(Rc<ChangeHandler>),
  /// Runs with a text entry's new selection.
  SelectionChange(Rc<SelectionHandler>),
  /// Runs with a control's new checked state.
  Toggle(Rc<ToggleHandler>),
  /// Runs when the platform dismisses the overlay.
  Dismiss(Rc<PlainHandler>),
  /// Builds what a drag from the node carries; the node's last is offered.
  DragStart(Rc<DragSource>),
  /// Answers whether the node takes a drag, and takes it once dropped.
  Drop(Rc<DropHandler>),
  /// Runs on a request for a context menu.
  ContextMenu(Rc<PointerHandler>),
}

impl Handler {
  /// The kind of event the handler runs on.
  #[inline(always)]
  pub const fn kind(&self) -> EventKind {
    match self {
      Self::Click(_) | Self::ActingClick(_) => EventKind::Click,
      Self::DoubleClick(_) => EventKind::DoubleClick,
      Self::KeyDown(_) => EventKind::KeyDown,
      Self::KeyUp(_) => EventKind::KeyUp,
      Self::PointerDown(_) => EventKind::PointerDown,
      Self::PointerUp(_) => EventKind::PointerUp,
      Self::PointerMove(_) => EventKind::PointerMove,
      Self::PointerCancel(_) => EventKind::PointerCancel,
      Self::PointerEnter(_) => EventKind::PointerEnter,
      Self::PointerLeave(_) => EventKind::PointerLeave,
      Self::Focus(_) => EventKind::Focus,
      Self::Blur(_) => EventKind::Blur,
      Self::Wheel(_) => EventKind::Wheel,
      Self::Scroll(_) => EventKind::Scroll,
      Self::Change(_) => EventKind::Change,
      Self::SelectionChange(_) => EventKind::SelectionChange,
      Self::Toggle(_) => EventKind::Toggle,
      Self::Dismiss(_) => EventKind::Dismiss,
      Self::DragStart(_) => EventKind::DragStart,
      Self::Drop(_) => EventKind::Drop,
      Self::ContextMenu(_) => EventKind::ContextMenu,
    }
  }
}

impl Debug for Handler {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_tuple("Handler")
      .field(&self.kind())
      .finish()
  }
}

/// A handler with the owner it was registered under, which it runs as: what it
/// creates, spawns or looks up in context belongs to the view that registered it.
#[derive(Debug)]
pub struct Registered<H: ?Sized> {
  owner: Owner,
  run: Rc<H>,
}

impl<H: ?Sized> Registered<H> {
  /// The owner the handler runs as.
  #[inline(always)]
  pub const fn owner(&self) -> Owner {
    self.owner
  }
}

impl<H: ?Sized> Clone for Registered<H> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      owner: self.owner,
      run: Rc::clone(&self.run),
    }
  }
}

impl<H: ?Sized> Deref for Registered<H> {
  type Target = H;

  #[inline(always)]
  fn deref(&self) -> &H {
    &self.run
  }
}

/// Picks one kind of handler out of a [`Handler`], typed.
pub(crate) type Pick<H> = fn(&Handler) -> Option<&Rc<H>>;

/// The handlers a node carries, in registration order, each with the owner it runs as.
#[derive(Default)]
pub(crate) struct HandlerSet {
  entries: Vec<(Owner, Handler)>,
}

impl HandlerSet {
  /// Adds `handler`, run as the current owner.
  pub(crate) fn push(&mut self, handler: Handler) {
    self.entries.push((Owner::current(), handler));
  }

  /// The handlers `pick` takes, in registration order.
  pub(crate) fn select<H: ?Sized>(&self, pick: Pick<H>) -> impl Iterator<Item = Registered<H>> {
    self.entries.iter().filter_map(move |(owner, handler)| {
      pick(handler).map(|run| Registered {
        owner: *owner,
        run: Rc::clone(run),
      })
    })
  }

  /// Whether `pick` takes any handler.
  pub(crate) fn has<H: ?Sized>(&self, pick: Pick<H>) -> bool {
    self
      .entries
      .iter()
      .any(|(_, handler)| pick(handler).is_some())
  }
}
