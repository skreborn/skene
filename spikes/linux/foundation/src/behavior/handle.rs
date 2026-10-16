//! The handle a control hands its behavior: its node, and the realization's operations on it
//! as plain functions.

use std::{rc::Rc, time::Duration};

use crate::{
  NodeId, Signal,
  layout::Rect,
  seam::{
    Realization,
    content::{Content, ContentKey, ContentType, PropValue},
    control::Part,
    runtime::Cancel,
    tree,
  },
};

/// What reads a content's value, erased.
pub type ContentReader = Rc<dyn Fn() -> PropValue>;

/// What hears a reader's change of a content's value, erased.
pub type ContentHandler = Rc<dyn Fn(PropValue)>;

/// The realization's half of a [`ControlHandle`], as plain functions the facade instantiates
/// once for its build's realization ([`ControlOps::of`]): monomorphized, never a `dyn`
/// realization.
#[derive(Clone, Copy)]
pub struct ControlOps {
  /// Moves focus to a node.
  pub focus: fn(NodeId),
  /// A control's first node recorded as a part.
  pub find_part: fn(NodeId, Part) -> Option<NodeId>,
  /// A node's rectangle in window pixels, as last laid out.
  pub frame: fn(NodeId) -> Option<Rect>,
  /// Runs an action once after a delay on the platform's clock.
  pub schedule: fn(Duration, Box<dyn FnOnce()>) -> Cancel,
  /// Keeps a content at what a closure returns.
  pub bind: fn(NodeId, Content, ContentReader),
  /// Makes a content controlled.
  pub control: fn(NodeId, Content, ContentReader, ContentHandler),
}

impl ControlOps {
  /// The operations of the realization `R`.
  #[must_use]
  pub const fn of<R: Realization>() -> Self {
    Self {
      focus: tree::focus::<R>,
      find_part: tree::find_part::<R>,
      frame: tree::frame::<R>,
      schedule: R::schedule,
      bind: tree::bind_value::<R>,
      control: tree::control_value::<R>,
    }
  }
}

/// What a control hands its behavior: its node, what its label names it, whether it is
/// disabled, and the realization's operations on it. Cloning shares it.
#[derive(Clone)]
#[must_use]
pub struct ControlHandle {
  node: NodeId,
  ops: ControlOps,
  name: Rc<dyn Fn() -> Option<Rc<str>>>,
  disabled: Rc<dyn Fn() -> bool>,
  refresh: Signal<u32>,
}

impl std::fmt::Debug for ControlHandle {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ControlHandle")
      .field("node", &self.node)
      .finish_non_exhaustive()
  }
}

impl ControlHandle {
  /// The handle of the control `node`, its realization's operations `ops`, named by what
  /// `name` returns and disabled where `disabled` says.
  pub fn new(
    node: NodeId,
    ops: ControlOps,
    name: impl Fn() -> Option<Rc<str>> + 'static,
    disabled: impl Fn() -> bool + 'static,
  ) -> Self {
    Self {
      node,
      ops,
      name: Rc::new(name),
      disabled: Rc::new(disabled),
      refresh: Signal::new(0),
    }
  }

  /// The control's node.
  #[inline(always)]
  #[must_use]
  pub const fn node(&self) -> NodeId {
    self.node
  }

  /// The accessible name its label supplies, as it reads now, tracking it.
  #[must_use]
  pub fn name(&self) -> Option<Rc<str>> {
    (self.name)()
  }

  /// Whether it is disabled, tracking it.
  #[must_use]
  pub fn is_disabled(&self) -> bool {
    (self.disabled)()
  }

  /// Moves focus to the control.
  pub fn focus(&self) {
    (self.ops.focus)(self.node);
  }

  /// Moves focus to another node, a member of the control.
  pub fn focus_node(&self, node: NodeId) {
    (self.ops.focus)(node);
  }

  /// The control's first node recorded as `part`; `Part::Root` the control's own.
  #[must_use]
  pub fn find_part(&self, part: Part) -> Option<NodeId> {
    (self.ops.find_part)(self.node, part)
  }

  /// `part`'s rectangle in window pixels as last laid out; `Part::Root` the control's own.
  #[must_use]
  pub fn frame(&self, part: Part) -> Option<Rect> {
    self.find_part(part).and_then(|node| (self.ops.frame)(node))
  }

  /// Runs `action` once after `delay` on the platform's clock.
  pub fn schedule(&self, delay: Duration, action: impl FnOnce() + 'static) -> Cancel {
    (self.ops.schedule)(delay, Box::new(action))
  }

  /// Keeps `key` at what `derive` returns, its value erased where the realization reads it.
  pub fn bind<T: ContentType>(&self, key: ContentKey<T>, derive: impl Fn() -> T + 'static) {
    (self.ops.bind)(
      self.node,
      key.content(),
      Rc::new(move || derive().into_value()),
    );
  }

  /// Controls `key`: the platform shows what `derive` returns, and each change a reader makes
  /// is asked of `handler`, the truth then shown.
  pub fn control<T: ContentType>(
    &self,
    key: ContentKey<T>,
    derive: impl Fn() -> T + 'static,
    handler: impl Fn(T) + 'static,
  ) {
    (self.ops.control)(
      self.node,
      key.content(),
      Rc::new(move || derive().into_value()),
      Rc::new(move |value| {
        if let Some(value) = T::from_value(&value) {
          handler(value);
        }
      }),
    );
  }

  /// Re-reads the control's reports (its state, its facts, its focus) for a change no signal
  /// carries.
  pub fn refresh(&self) {
    self.refresh.update(|count| *count = count.wrapping_add(1));
  }

  /// Subscribes the running effect to [`ControlHandle::refresh`].
  pub fn track_refresh(&self) {
    let _ = self.refresh.get();
  }
}
