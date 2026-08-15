//! The surface every form control shares: styling, conditions, dynamics, and focus.

// Fields are read only by a platform's realization; a target with no platform compiles
// the base but never reads them.

use std::rc::Rc;

use skene_core::{
  Animation,
  BoxStyle,
  Condition,
  KeyEvent,
  Override,
  PointerEvent,
  Prevent,
  ScrollEvent,
  Transition,
};

use crate::{Container, FocusHandle};

/// The focus state behind a control's shared builder surface: the target of
/// [`HasFocus`](crate::HasFocus).
pub struct FocusCore {
  pub(crate) focus_order: Option<skene_core::FocusOrder>,
  pub(crate) autofocus: bool,
  pub(crate) focus_handle: Option<FocusHandle>,
  pub(crate) anchor: Option<crate::Anchor>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
  pub(crate) key_down_handler: Option<Rc<skene_core::KeyHandler>>,
}

/// The styling state behind a control's shared builder surface: the target of
/// [`HasStyle`](crate::HasStyle).
pub struct StyleCore {
  pub(crate) style: BoxStyle,
  pub(crate) variants: Vec<(Condition, BoxStyle)>,
  pub(crate) transitions: Vec<Transition>,
  pub(crate) animations: Vec<Animation<BoxStyle>>,
}

/// The event state behind a drawn control's shared builder surface: the target of
/// [`HasEvents`](crate::HasEvents).
///
/// The preventable events are [`Override`]s, so a handler may take one from the control
/// outright; the notification events carry no verdict and only observe.
#[derive(Default)]
pub struct EventCore {
  pub(crate) click: Override<()>,
  pub(crate) key_down: Override<KeyEvent>,
  pub(crate) pointer_down: Override<PointerEvent>,
  pub(crate) pointer_move: Override<PointerEvent>,
  pub(crate) pointer_up: Override<PointerEvent>,
  pub(crate) pointer_enter: Option<Rc<Observed<()>>>,
  pub(crate) pointer_leave: Option<Rc<Observed<()>>>,
  pub(crate) focus: Option<Rc<Observed<()>>>,
  pub(crate) blur: Option<Rc<Observed<()>>>,
  pub(crate) scroll: Option<Rc<Observed<ScrollEvent>>>,
}

/// What a drawn control does of its own accord, for the events it acts on.
///
/// A control supplies one of these to [`EventCore::attach`]; every event left unset
/// simply reports its handler's verdict, so a control pays nothing for the events it
/// ignores and gains the override path for free when it later acts on one.
#[derive(Default)]
#[must_use]
pub struct Behavior {
  click: Option<ClickBehavior>,
  key_down: Option<BehaviorFor<KeyEvent>>,
  pointer_down: Option<BehaviorFor<PointerEvent>>,
  pointer_move: Option<BehaviorFor<PointerEvent>>,
  pointer_up: Option<BehaviorFor<PointerEvent>>,
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

  /// Sets what the control does on a pointer press.
  #[inline(always)]
  pub fn pointer_down(mut self, behavior: impl Fn(&PointerEvent) -> Prevent + 'static) -> Self {
    self.pointer_down = Some(Box::new(behavior));

    self
  }

  /// Sets what the control does on a pointer move.
  #[inline(always)]
  pub fn pointer_move(mut self, behavior: impl Fn(&PointerEvent) -> Prevent + 'static) -> Self {
    self.pointer_move = Some(Box::new(behavior));

    self
  }

  /// Sets what the control does on a pointer release.
  #[inline(always)]
  pub fn pointer_up(mut self, behavior: impl Fn(&PointerEvent) -> Prevent + 'static) -> Self {
    self.pointer_up = Some(Box::new(behavior));

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
  /// Registers every handler on `container`, each installed one folded over what
  /// `behavior` supplies for that event.
  pub fn attach(self, container: Container, behavior: Behavior) -> Container {
    let Behavior {
      click,
      key_down,
      pointer_down,
      pointer_move,
      pointer_up,
      pointer_enter,
      pointer_leave,
      focus,
      blur,
      scroll,
    } = behavior;
    let click = self
      .click
      .wrap(move |&()| click.as_ref().map_or(Prevent::None, |act| act()));
    let mut container = container
      .handle_click(move || click(&()))
      .handle_key_down(
        self
          .key_down
          .wrap(move |event| key_down.as_ref().map_or(Prevent::None, |act| act(event))),
      )
      .handle_pointer_down(self.pointer_down.wrap(move |event| {
        pointer_down
          .as_ref()
          .map_or(Prevent::None, |act| act(event))
      }))
      .handle_pointer_move(self.pointer_move.wrap(move |event| {
        pointer_move
          .as_ref()
          .map_or(Prevent::None, |act| act(event))
      }))
      .handle_pointer_up(
        self
          .pointer_up
          .wrap(move |event| pointer_up.as_ref().map_or(Prevent::None, |act| act(event))),
      );

    if let Some(action) = observed(pointer_enter, self.pointer_enter) {
      container = container.handle_pointer_enter(move || action(&()));
    }

    if let Some(action) = observed(pointer_leave, self.pointer_leave) {
      container = container.handle_pointer_leave(move || action(&()));
    }

    if let Some(action) = observed(focus, self.focus) {
      container = container.handle_focus(move || action(&()));
    }

    if let Some(action) = observed(blur, self.blur) {
      container = container.handle_blur(move || action(&()));
    }

    if let Some(action) = observed(scroll, self.scroll) {
      container = container.handle_scroll(move |event| action(event));
    }

    container
  }
}

/// The state behind a control's shared builder surface, embedded by every control.
pub(crate) struct ControlBase {
  pub(crate) style: StyleCore,
  pub(crate) focus: FocusCore,
}

impl ControlBase {
  pub(crate) fn new() -> Self {
    Self {
      style: StyleCore {
        style: BoxStyle::new(),
        variants: Vec::new(),
        transitions: Vec::new(),
        animations: Vec::new(),
      },
      focus: FocusCore {
        focus_order: None,
        autofocus: false,
        focus_handle: None,
        anchor: None,
        focus_handler: None,
        blur_handler: None,
        key_down_handler: None,
      },
    }
  }

  /// Splits the base into the realization props and the handlers to register on the
  /// built node, resolving the inherited text style into the props' static text and
  /// the handlers' reactive binders.
  pub(crate) fn into_parts(self) -> (skene_core::ControlProps, ControlHandlers) {
    use skene_core::{RunProp, RunStyle};

    use crate::realize::split_prop;

    let resolved = crate::text::resolved_text_style(crate::TextStyle::new(), false);
    let mut text = RunStyle::default();
    let mut binders = crate::realize::Binders::new();

    text.selection_color = resolved.selection_color;
    text.selection_background = resolved.selection_background;

    split_prop(
      resolved.color,
      &mut text.color,
      &mut binders,
      RunProp::Color,
      crate::driver::update_run,
    );

    split_prop(
      resolved.font_size,
      &mut text.font_size,
      &mut binders,
      RunProp::FontSize,
      crate::driver::update_run,
    );

    split_prop(
      resolved.font_weight,
      &mut text.font_weight,
      &mut binders,
      RunProp::FontWeight,
      crate::driver::update_run,
    );

    (
      skene_core::ControlProps {
        style: self.style.style,
        text,
        variants: self.style.variants,
        transitions: self.style.transitions,
        animations: self.style.animations,
        focus_order: self.focus.focus_order,
        autofocus: self.focus.autofocus,
      },
      ControlHandlers {
        focus_handle: self.focus.focus_handle,
        anchor: self.focus.anchor,
        focus: self.focus.focus_handler,
        blur: self.focus.blur_handler,
        key_down: self.focus.key_down_handler,
        binders,
      },
    )
  }
}

/// The shared handlers split off a [`ControlBase`], registered once the node exists.
pub(crate) struct ControlHandlers {
  focus_handle: Option<FocusHandle>,
  anchor: Option<crate::Anchor>,
  focus: Option<Rc<dyn Fn()>>,
  blur: Option<Rc<dyn Fn()>>,
  key_down: Option<Rc<skene_core::KeyHandler>>,
  binders: crate::realize::Binders,
}

impl ControlHandlers {
  /// Registers the handlers on `node`, runs the reactive text-style binders, and binds
  /// the focus handle to it.
  pub(crate) fn register(self, node: skene_core::NodeId) {
    for binder in self.binders {
      binder(node);
    }

    if let Some(action) = self.focus {
      crate::driver::handle_focus(node, action);
    }

    if let Some(action) = self.blur {
      crate::driver::handle_blur(node, action);
    }

    if let Some(action) = self.key_down {
      crate::driver::handle_key_down(node, action);
    }

    if let Some(handle) = &self.focus_handle {
      handle.assign(node);
    }

    if let Some(anchor) = &self.anchor {
      anchor.assign(node);
    }
  }
}

/// Wraps a built toggle control in a label box carrying its caption, so activating the
/// caption activates the control; returns the wrapper.
pub(crate) fn labeled(control: skene_core::NodeId, label: String) -> skene_core::NodeId {
  use skene_core::{BoxProps, BoxStyle, FlexDirection, ItemAlignment, Layout, View};

  let wrapper = crate::driver::create_box(&BoxProps {
    layout: Layout::Flex {
      direction: FlexDirection::Row,
    },
    role: Some(skene_core::Role::Label),
    style: BoxStyle::new()
      .set_spacing(8.)
      .set_item_alignment(ItemAlignment::Center),
    variants: Vec::new(),
    transitions: Vec::new(),
    animations: Vec::new(),
  });

  crate::driver::attach(wrapper, control, None);

  let caption = crate::Text::new(label).build();

  crate::driver::attach(wrapper, caption, None);

  wrapper
}
