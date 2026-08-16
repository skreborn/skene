//! The surface every form control shares: styling, conditions, dynamics, and focus.

// Fields are read only by a platform's realization; a target with no platform compiles
// the base but never reads them.

use std::rc::Rc;

use skene_core::{
  Animation,
  BoxStyle,
  Condition,
  KeyEvent,
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
/// Every handler here only observes: a control owns its own behavior, and nothing a
/// user installs displaces it. What the dispatch is told to prevent is the control's
/// decision alone, so no handler above the driver states one.
#[derive(Default)]
pub struct EventCore {
  pub(crate) click: Option<Rc<Observed<()>>>,
  pub(crate) key_down: Option<Rc<Observed<KeyEvent>>>,
  pub(crate) pointer_down: Option<Rc<Observed<PointerEvent>>>,
  pub(crate) pointer_move: Option<Rc<Observed<PointerEvent>>>,
  pub(crate) pointer_up: Option<Rc<Observed<PointerEvent>>>,
  pub(crate) pointer_enter: Option<Rc<Observed<()>>>,
  pub(crate) pointer_leave: Option<Rc<Observed<()>>>,
  pub(crate) focus: Option<Rc<Observed<()>>>,
  pub(crate) blur: Option<Rc<Observed<()>>>,
  pub(crate) scroll: Option<Rc<Observed<ScrollEvent>>>,
}

/// What a drawn control does of its own accord, and what that costs the dispatch.
///
/// A control supplies one of these to [`EventCore::attach`]. Its verdict is the whole
/// verdict: an event a control acts on is an event it consumes, so the control—not its
/// user—decides what the platform and the ancestors see.
#[derive(Default)]
#[must_use]
pub struct Behavior {
  click: Option<BehaviorFor<()>>,
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
/// A handler that only observes an event, as both sides store it.
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
    self.click = Some(Box::new(move |&()| behavior()));

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

/// Pairs a control's behavior for one notification event with the handler installed
/// over it: both run, the control's first. `None` when neither exists, so an untouched
/// event registers nothing.
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
  /// Registers every handler on `container`, each over what `behavior` supplies for
  /// that event.
  pub fn attach(self, container: Container, behavior: Behavior) -> Container {
    let click = acted(behavior.click, self.click);
    let mut container = container;

    // A control states a full verdict, which the box's own surface no longer spells: a
    // handler there decides bubbling and nothing else.
    container.node.click_handler = Some(Rc::new(move || click(&())));
    container.node.key_down_handler = Some(Rc::new(acted(behavior.key_down, self.key_down)));
    container.node.pointer_down_handler =
      Some(Rc::new(acted(behavior.pointer_down, self.pointer_down)));
    container.node.pointer_move_handler =
      Some(Rc::new(acted(behavior.pointer_move, self.pointer_move)));
    container.node.pointer_up_handler = Some(Rc::new(acted(behavior.pointer_up, self.pointer_up)));

    if let Some(action) = observed(behavior.pointer_enter, self.pointer_enter) {
      container = container.handle_pointer_enter(move || action(&()));
    }

    if let Some(action) = observed(behavior.pointer_leave, self.pointer_leave) {
      container = container.handle_pointer_leave(move || action(&()));
    }

    if let Some(action) = observed(behavior.focus, self.focus) {
      container = container.handle_focus(move || action(&()));
    }

    if let Some(action) = observed(behavior.blur, self.blur) {
      container = container.handle_blur(move || action(&()));
    }

    if let Some(action) = observed(behavior.scroll, self.scroll) {
      container = container.handle_scroll(move |event| action(event));
    }

    container
  }

  /// Registers every handler on the built `node`, each over what `behavior` supplies
  /// for that event. The path for a control the driver realizes, which has no
  /// [`Container`] to build through.
  pub fn register(self, node: skene_core::NodeId, behavior: Behavior) {
    use crate::driver;

    let click = acted(behavior.click, self.click);

    driver::handle_click(node, Rc::new(move || click(&())));
    driver::handle_key_down(node, Rc::new(acted(behavior.key_down, self.key_down)));
    driver::handle_pointer_down(
      node,
      Rc::new(acted(behavior.pointer_down, self.pointer_down)),
    );
    driver::handle_pointer_move(
      node,
      Rc::new(acted(behavior.pointer_move, self.pointer_move)),
    );
    driver::handle_pointer_up(node, Rc::new(acted(behavior.pointer_up, self.pointer_up)));

    if let Some(action) = observed(behavior.pointer_enter, self.pointer_enter) {
      driver::handle_pointer_enter(node, Rc::new(move || action(&())));
    }

    if let Some(action) = observed(behavior.pointer_leave, self.pointer_leave) {
      driver::handle_pointer_leave(node, Rc::new(move || action(&())));
    }

    if let Some(action) = observed(behavior.focus, self.focus) {
      driver::handle_focus(node, Rc::new(move || action(&())));
    }

    if let Some(action) = observed(behavior.blur, self.blur) {
      driver::handle_blur(node, Rc::new(move || action(&())));
    }

    if let Some(action) = observed(behavior.scroll, self.scroll) {
      driver::handle_scroll(node, Rc::new(move |event| action(event)));
    }
  }
}

/// The state behind a control's shared builder surface, embedded by every control.
pub(crate) struct ControlBase {
  pub(crate) style: StyleCore,
  pub(crate) focus: FocusCore,
  pub(crate) events: EventCore,
}

impl ControlBase {
  pub(crate) fn new() -> Self {
    Self {
      events: EventCore::default(),
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
        events: self.events,
        binders,
      },
    )
  }
}

/// The shared handlers split off a [`ControlBase`], registered once the node exists.
pub(crate) struct ControlHandlers {
  focus_handle: Option<FocusHandle>,
  anchor: Option<crate::Anchor>,
  events: EventCore,
  binders: crate::realize::Binders,
}

impl ControlHandlers {
  /// Registers the handlers on `node`, runs the reactive text-style binders, and binds
  /// the focus handle to it.
  pub(crate) fn register(self, node: skene_core::NodeId) {
    for binder in self.binders {
      binder(node);
    }

    self.events.register(node, Behavior::new());

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
