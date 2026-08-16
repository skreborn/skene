//! The surface every terminal control shares: styling, conditions, and focus.

use std::rc::Rc;

use skene_core::{KeyEvent, NodeId, Prevent};

use crate::{FocusHandle, events::ScrollEvent, props::Condition, with_state};

/// The focus state behind a control's shared builder surface: the target of
/// [`HasFocus`](crate::HasFocus).
pub struct FocusCore {
  pub(crate) autofocus: bool,
  pub(crate) focus_handle: Option<FocusHandle>,
  pub(crate) anchor: Option<crate::Anchor>,
}

/// The event state behind a control's shared builder surface: the target of
/// [`HasEvents`](crate::HasEvents).
///
/// Every handler here only observes: a control owns its own behavior, and nothing a
/// user installs displaces it. What the dispatch is told to prevent is the control's
/// decision alone, so no handler above the driver states one.
#[derive(Default)]
pub struct EventCore {
  pub(crate) click: Option<Rc<Observed<()>>>,
  pub(crate) key_down: Option<Rc<Observed<KeyEvent>>>,
  pub(crate) pointer_enter: Option<Rc<Observed<()>>>,
  pub(crate) pointer_leave: Option<Rc<Observed<()>>>,
  pub(crate) focus: Option<Rc<Observed<()>>>,
  pub(crate) blur: Option<Rc<Observed<()>>>,
  pub(crate) scroll: Option<Rc<Observed<ScrollEvent>>>,
}

/// What a terminal control does of its own accord, for the events it acts on.
///
/// A control supplies one of these to [`EventCore::register`]. Its verdict is the whole
/// verdict: an event a control acts on is an event it consumes. A control the driver
/// realizes acts on none of them—its behavior is the terminal's own, which
/// [`Prevent::Platform`] would suppress, so it never asks for that.
#[derive(Default)]
#[must_use]
pub struct Behavior {
  click: Option<BehaviorFor<()>>,
  key_down: Option<BehaviorFor<KeyEvent>>,
  pointer_enter: Option<Rc<Observed<()>>>,
  pointer_leave: Option<Rc<Observed<()>>>,
  focus: Option<Rc<Observed<()>>>,
  blur: Option<Rc<Observed<()>>>,
  scroll: Option<Rc<Observed<ScrollEvent>>>,
}

/// What a control does on one event carrying `E`, as [`Behavior`] stores it.
type BehaviorFor<E> = Box<dyn Fn(&E) -> Prevent>;
/// A handler that only observes an event, as both sides store it.
type Observed<E> = dyn Fn(&E);

/// Folds a control's behavior for one event together with the handler installed over
/// it: the control acts first, then the handler observes what it did, and the verdict
/// is the control's.
fn acted<E: ?Sized + 'static>(
  behavior: Option<BehaviorFor<E>>,
  handler: Option<Rc<Observed<E>>>,
) -> impl Fn(&E) -> Prevent + 'static {
  move |event| {
    let prevented = behavior.as_ref().map_or(Prevent::None, |act| act(event));

    if let Some(handler) = &handler {
      handler(event);
    }

    prevented
  }
}

impl Behavior {
  /// Creates a behavior that acts on nothing.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets what the control does on a click.
  #[inline(always)]
  pub fn click(mut self, behavior: impl Fn() -> Prevent + 'static) -> Self {
    self.click = Some(Box::new(move |&()| behavior()));

    self
  }

  /// Sets what the control does on a key press.
  #[inline(always)]
  pub fn key_down(mut self, behavior: impl Fn(&KeyEvent) -> Prevent + 'static) -> Self {
    self.key_down = Some(Box::new(behavior));

    self
  }

  /// Sets what the control does when the pointer enters it.
  #[inline(always)]
  pub fn pointer_enter(mut self, behavior: impl Fn() + 'static) -> Self {
    self.pointer_enter = Some(Rc::new(move |&()| behavior()));

    self
  }

  /// Sets what the control does when the pointer leaves it.
  #[inline(always)]
  pub fn pointer_leave(mut self, behavior: impl Fn() + 'static) -> Self {
    self.pointer_leave = Some(Rc::new(move |&()| behavior()));

    self
  }

  /// Sets what the control does when focus arrives.
  #[inline(always)]
  pub fn focus(mut self, behavior: impl Fn() + 'static) -> Self {
    self.focus = Some(Rc::new(move |&()| behavior()));

    self
  }

  /// Sets what the control does when focus leaves.
  #[inline(always)]
  pub fn blur(mut self, behavior: impl Fn() + 'static) -> Self {
    self.blur = Some(Rc::new(move |&()| behavior()));

    self
  }

  /// Sets what the control does on a scroll.
  #[inline(always)]
  pub fn scroll(mut self, behavior: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.scroll = Some(Rc::new(behavior));

    self
  }
}

/// Pairs a control's behavior for one notification event with the handler installed
/// over it.
///
/// A notification takes no verdict, so nothing can be taken over: both run, the
/// control's first, and the installed handler observes what it did. `None` when neither
/// exists, so an untouched event registers nothing.
fn observed<E: ?Sized + 'static>(
  behavior: Option<Rc<Observed<E>>>,
  handler: Option<Rc<Observed<E>>>,
) -> Option<impl Fn(&E) + 'static> {
  (behavior.is_some() || handler.is_some()).then_some({
    move |event: &E| {
      if let Some(behavior) = &behavior {
        behavior(event);
      }

      if let Some(handler) = &handler {
        handler(event);
      }
    }
  })
}

impl EventCore {
  /// Registers every handler on the built `node`, each installed one folded over what
  /// `behavior` supplies for that event.
  pub fn register(self, node: NodeId, behavior: Behavior) {
    let Behavior {
      click,
      key_down,
      pointer_enter,
      pointer_leave,
      focus,
      blur,
      scroll,
    } = behavior;
    let clicked = acted(click, self.click);
    let pressed = acted(key_down, self.key_down);

    with_state(|state| {
      state
        .click
        .borrow_mut()
        .entry(node)
        .or_default()
        .push(Rc::new(move || clicked(&())));

      state
        .key_down
        .borrow_mut()
        .entry(node)
        .or_default()
        .push(Rc::new(pressed));

      if let Some(action) = observed(pointer_enter, self.pointer_enter) {
        state
          .pointer_enter
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(Rc::new(move || action(&())));
      }

      if let Some(action) = observed(pointer_leave, self.pointer_leave) {
        state
          .pointer_leave
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(Rc::new(move || action(&())));
      }

      if let Some(action) = observed(focus, self.focus) {
        state
          .focus
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(Rc::new(move || action(&())));
      }

      if let Some(action) = observed(blur, self.blur) {
        state
          .blur
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(Rc::new(move || action(&())));
      }

      if let Some(action) = observed(scroll, self.scroll) {
        state
          .scroll
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(Rc::new(move |event: &ScrollEvent| action(event)));
      }
    });
  }
}

/// The decoration state behind a control's shared builder surface; `S` is the
/// vocabulary the control decorates in.
pub struct Decoration<S> {
  pub(crate) style: S,
  pub(crate) variants: Vec<(Condition, S)>,
}

/// What a control that draws marks decorates: the target of [`HasMarks`](crate::HasMarks).
pub type MarkCore = Decoration<crate::props::ControlStyle>;
/// What a control owning a box decorates: the target of
/// [`HasDecoration`](crate::HasDecoration).
pub type DecorationCore = Decoration<crate::props::BoxStyle>;

/// The state behind a control's shared builder surface, embedded by every control;
/// `S` is the style vocabulary the control accepts (a small control's
/// [`ControlStyle`](crate::props::ControlStyle), a container-owning one's
/// [`BoxStyle`](crate::props::BoxStyle)).
pub(crate) struct ControlBase<S> {
  pub(crate) style: Decoration<S>,
  pub(crate) focus: FocusCore,
  pub(crate) events: EventCore,
}

impl<S: Default> ControlBase<S> {
  pub(crate) fn new() -> Self {
    Self {
      style: Decoration {
        style: S::default(),
        variants: Vec::new(),
      },
      focus: FocusCore {
        autofocus: false,
        focus_handle: None,
        anchor: None,
      },
      events: EventCore::default(),
    }
  }

  /// Registers the built control: joins the focus order, wires the base's handlers over
  /// what `behavior` supplies, and binds its handles.
  pub(crate) fn register(self, node: NodeId, behavior: Behavior) {
    let focus = self.focus;

    self.events.register(node, behavior);

    with_state(|state| {
      state.controls.borrow_mut().insert(node);

      if focus.autofocus {
        state.focused.set(Some(node));
        state.dirty.set(true);
      }
    });

    if let Some(handle) = &focus.focus_handle {
      handle.assign(node);
    }

    if let Some(anchor) = &focus.anchor {
      anchor.assign(node);
    }
  }
}
