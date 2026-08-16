//! The surface every terminal control shares: styling, conditions, and focus.

use std::rc::Rc;

use skene_core::{KeyEvent, NodeId, Override, Prevent};

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
/// The preventable events are [`Override`]s, so a handler may take one from the control
/// outright; the notification events carry no verdict and only observe. A terminal
/// delivers no separate pointer press, move, and release to a control, so its
/// preventable pair is the click and the key.
#[derive(Default)]
pub struct EventCore {
  pub(crate) click: Override<()>,
  pub(crate) key_down: Override<KeyEvent>,
  pub(crate) pointer_enter: Option<Rc<Observed<()>>>,
  pub(crate) pointer_leave: Option<Rc<Observed<()>>>,
  pub(crate) focus: Option<Rc<Observed<()>>>,
  pub(crate) blur: Option<Rc<Observed<()>>>,
  pub(crate) scroll: Option<Rc<Observed<ScrollEvent>>>,
}

/// What a terminal control does of its own accord, for the events it acts on.
///
/// A control supplies one of these to [`EventCore::register`]; every event left unset
/// simply reports its handler's verdict. A control the driver realizes acts on none of
/// them—its behavior is the terminal's own, and [`Prevent::Platform`] suppresses that.
#[derive(Default)]
#[must_use]
pub struct Behavior {
  click: Option<ClickBehavior>,
  key_down: Option<BehaviorFor<KeyEvent>>,
  pointer_enter: Option<Rc<Observed<()>>>,
  pointer_leave: Option<Rc<Observed<()>>>,
  focus: Option<Rc<Observed<()>>>,
  blur: Option<Rc<Observed<()>>>,
  scroll: Option<Rc<Observed<ScrollEvent>>>,
}

/// What a control does on one event carrying `E`, as [`Behavior`] stores it.
type BehaviorFor<E> = Box<dyn Fn(&E) -> Prevent>;
/// What a control does on a click, which carries nothing.
type ClickBehavior = Box<dyn Fn() -> Prevent>;
/// What a control does on one notification event, which takes no verdict.
type Observed<E> = dyn Fn(&E);

impl Behavior {
  /// Creates a behavior that acts on nothing.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets what the control does on a click.
  #[inline(always)]
  pub fn click(mut self, behavior: impl Fn() -> Prevent + 'static) -> Self {
    self.click = Some(Box::new(behavior));

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
    let clicked = self
      .click
      .wrap(move |&()| click.as_ref().map_or(Prevent::None, |act| act()));
    let pressed = self
      .key_down
      .wrap(move |event| key_down.as_ref().map_or(Prevent::None, |act| act(event)));

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

/// The styling state behind a control's shared builder surface: the target of
/// [`HasStyle`](crate::HasStyle); `S` is the style vocabulary the control accepts.
pub struct StyleCore<S> {
  pub(crate) style: S,
  pub(crate) variants: Vec<(Condition, S)>,
}

/// The state behind a control's shared builder surface, embedded by every control;
/// `S` is the style vocabulary the control accepts (a small control's
/// [`ControlStyle`](crate::props::ControlStyle), a container-owning one's
/// [`BoxStyle`](crate::props::BoxStyle)).
pub(crate) struct ControlBase<S> {
  pub(crate) style: StyleCore<S>,
  pub(crate) focus: FocusCore,
  pub(crate) events: EventCore,
}

impl<S: Default> ControlBase<S> {
  pub(crate) fn new() -> Self {
    Self {
      style: StyleCore {
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
