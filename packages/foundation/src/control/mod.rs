//! The one control builder, the behavior it is given, and the behaviors that ship.

mod activate;
mod choice;
mod collection;
mod controlled;
mod native_picker;
mod ops;
mod popup;
mod roster;
mod slider;
mod text_entry;
mod toggle;
mod type_ahead;

use std::rc::Rc;

use skene_reactive::{Effect, Get, Owner, Set, Signal, Trigger, untrack};

pub use self::{
  activate::Activate,
  choice::{AnyChoice, AnyChosen, Choice, Chosen, ControlledChoice},
  collection::{
    Collection,
    CollectionContext,
    CollectionPolicy,
    Listbox,
    ListboxContext,
    ListboxOption,
    ListboxPolicy,
    Member,
    Menu,
    MenuContext,
    MenuOption,
    MenuPolicy,
    Radio,
    RadioGroup,
    RadioGroupContext,
    RadioPolicy,
    Ring,
  },
  controlled::Controlled,
  native_picker::NativePicker,
  ops::ControlOps,
  popup::Popup,
  roster::Roster,
  slider::{Slider, SliderContext, SliderRange},
  text_entry::TextEntry,
  toggle::Toggle,
  type_ahead::{TYPE_AHEAD_RESET, TypeAhead},
};
use crate::{
  AccessProp,
  Alignment,
  AnyKind,
  AnyView,
  Axis,
  ClickEvent,
  Condition,
  ContainerName,
  ControlFacts,
  ControlState,
  Controls,
  DeclaredBoxStyle,
  DecorationBase,
  DragAxes,
  EventKind,
  Expr,
  FocusOrder,
  Handler,
  HasBoxStyle,
  HasLayoutBox,
  ItemAlignment,
  KeyEvent,
  Keyboard,
  Layout,
  LayoutBox,
  MotionBase,
  NativeElement,
  NodeFacts,
  NodeHandle,
  NodeId,
  Part,
  PlainChild,
  PointerEvent,
  Prevent,
  Prevented,
  Realization,
  Role,
  Scene,
  ScrollEvent,
  SelectionColors,
  Shortcut,
  Spacing,
  StaticLeaf,
  Text,
  TextSelection,
  Thin,
  Value,
  View,
  WheelEvent,
  Wraps,
  realize::{Source, report_access},
};

/// A handler of an event carrying `E`, with its verdict.
type Acting<E> = dyn Fn(&E) -> Prevent;

/// Which pointer transition a behavior is told of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum PointerPhase {
  /// A button pressed over the control.
  Down,
  /// The pointer moved over the control.
  Move,
  /// A button released over the control.
  Up,
  /// The platform took the pointer pressed over the control for itself, ending the
  /// press without a release.
  Cancel,
}

/// Where a control's label sits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LabelPlacement {
  /// Inside the control's row, before its parts and body, as a group's caption.
  Leading,
  /// Inside the control's row, after its parts and body, as a mark's caption.
  #[default]
  Trailing,
  /// Before the control's box among its siblings, outside its decoration, answering
  /// to the control, as a text entry's caption.
  Beside,
}

/// What a control's interaction is: its kind and role, the events it acts on and what
/// it does with them, and what it reports.
///
/// A [`Control`] runs its behavior first on every event it acts on, and the behavior's
/// verdict is the whole verdict; the control's own handlers observe. [`access`],
/// [`focus_order`] and [`state`] are read under render effects, so what they read from
/// signals re-applies when it changes. Only the events in [`listens`] and those the
/// control's handlers want are registered.
///
/// A behavior names no realization: every hook is given the control's
/// [`ControlHandle`], which reaches the build's one realization through plain functions
/// ([`ControlOps`]), so a third party's behavior is written as the shipped ones are.
///
/// [`access`]: Self::access
/// [`focus_order`]: Self::focus_order
/// [`state`]: Self::state
/// [`listens`]: Self::listens
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a control behavior",
  label = "a behavior states a control's kind, role, the events it acts on, and what it reports"
)]
pub trait Behavior: 'static {
  /// The native element the control stands in, where a platform has one; `None`, the
  /// default, for a control its view draws.
  #[inline(always)]
  fn native(&self) -> Option<NativeElement> {
    None
  }

  /// The semantic role the control's node carries; `None` where the native element says
  /// enough.
  fn role(&self) -> Option<Role>;

  /// The events the behavior acts on.
  fn listens(&self) -> &'static [EventKind];

  /// The lines a text entry shows at once.
  #[inline(always)]
  fn lines(&self) -> u16 {
    1
  }

  /// The axes a press's pointer drags the control along, as a slider's; a platform
  /// keeps the pointer with the control until the release and a touch from panning
  /// along them. `None` for a control no press drags.
  #[inline(always)]
  fn drag_axes(&self) -> Option<DragAxes> {
    None
  }

  /// Where the control's label sits: beside the box of a control whose box holds a
  /// value—a text entry, a picker—and after its parts and body inside it otherwise.
  #[inline(always)]
  fn label_placement(&self) -> LabelPlacement {
    match self.native() {
      Some(native) if native.is_text_entry() || native == NativeElement::Picker => {
        LabelPlacement::Beside
      }
      _ => LabelPlacement::Trailing,
    }
  }

  /// Runs once the control's node exists and its reports have been read, before its
  /// parts and child build, under the control's owner: what a behavior binds to the
  /// node, provides to its subtree or joins goes here, and a content it controls is
  /// registered here ([`ControlHandle::register`]). A behavior whose reports change here
  /// calls [`ControlHandle::refresh`].
  #[inline(always)]
  fn attach(&self, _control: &ControlHandle) {}

  /// Runs once the control's parts and child are in place, under the control's owner:
  /// what a behavior finds among its parts, and measures, goes here.
  #[inline(always)]
  fn mounted(&self, _control: &ControlHandle) {}

  /// What the behavior does on a click.
  #[inline(always)]
  fn on_click(&self, _control: &ControlHandle) -> Prevent {
    Prevent::None
  }

  /// What the behavior does on a key pressed while the control holds focus.
  #[inline(always)]
  fn on_key(&self, _event: &KeyEvent, _control: &ControlHandle) -> Prevent {
    Prevent::None
  }

  /// What the behavior does on a key released while the control holds focus.
  #[inline(always)]
  fn on_key_up(&self, _event: &KeyEvent, _control: &ControlHandle) -> Prevent {
    Prevent::None
  }

  /// What the behavior does when the control loses focus.
  #[inline(always)]
  fn on_blur(&self, _control: &ControlHandle) {}

  /// What the behavior does on a wheel turned over the control, before the platform
  /// scrolls by it; suppressing the platform's response keeps the scroll from
  /// happening.
  #[inline(always)]
  fn on_wheel(&self, _event: &WheelEvent, _control: &ControlHandle) -> Prevent {
    Prevent::None
  }

  /// What the behavior does on a pointer transition over the control.
  #[inline(always)]
  fn on_pointer(
    &self,
    _phase: PointerPhase,
    _event: &PointerEvent,
    _control: &ControlHandle,
  ) -> Prevent {
    Prevent::None
  }

  /// What the control reports to assistive technology, beside its role.
  #[inline(always)]
  fn access(&self) -> Vec<AccessProp> {
    Vec::new()
  }

  /// The control's place in focus traversal; `None` for a control that takes no focus.
  #[inline(always)]
  fn focus_order(&self) -> Option<FocusOrder> {
    Some(FocusOrder::Auto)
  }

  /// The state the control-scoped conditions inside the control resolve against.
  #[inline(always)]
  fn state(&self) -> ControlState {
    ControlState::DEFAULT
  }
}

/// A behavior's access to the control it drives.
///
/// It holds the node, its name and disabled state, focus, a click dispatched at it, its
/// parts, the registration of what it controls, a re-read of its reports, and the
/// build's realization's operations ([`ControlOps`]) for what it does beyond its own
/// node.
#[must_use]
pub struct ControlHandle {
  node: NodeId,
  name: Option<Rc<dyn Fn() -> String>>,
  disabled: Rc<dyn Fn() -> bool>,
  changed: Trigger,
  ops: ControlOps,
}

impl std::fmt::Debug for ControlHandle {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ControlHandle")
      .finish_non_exhaustive()
  }
}

impl ControlHandle {
  /// The control's node.
  #[inline(always)]
  pub const fn node(&self) -> NodeId {
    self.node
  }

  /// The accessible name the control's label supplies, as it reads now.
  #[inline(always)]
  #[must_use]
  pub fn name(&self) -> Option<String> {
    self.name.as_ref().map(|name| name())
  }

  /// [`name`](Self::name) as a read that outlives the handle, for a roster the control
  /// joins.
  #[inline(always)]
  #[must_use]
  pub fn name_read(&self) -> Option<Rc<dyn Fn() -> String>> {
    self.name.clone()
  }

  /// Whether the control is disabled; tracks where the control derives it.
  #[inline(always)]
  #[must_use]
  pub fn is_disabled(&self) -> bool {
    (self.disabled)()
  }

  /// [`is_disabled`](Self::is_disabled) as a read that outlives the handle, for a
  /// roster the control joins.
  #[inline(always)]
  #[must_use]
  pub fn disabled_read(&self) -> Rc<dyn Fn() -> bool> {
    Rc::clone(&self.disabled)
  }

  /// The operations of the build's realization, for what the behavior does beyond the
  /// control's own node.
  #[inline(always)]
  pub const fn ops(&self) -> ControlOps {
    self.ops
  }

  /// Moves focus to the control.
  #[inline(always)]
  pub fn focus(&self) {
    self.ops.focus(self.node);
  }

  /// Dispatches `event` as a click at the control, so its behavior and handlers see it.
  #[inline(always)]
  pub fn click(&self, event: &ClickEvent) -> Prevented {
    self.ops.click(self.node, event)
  }

  /// The control's first node recorded as `part`.
  #[inline(always)]
  #[must_use]
  pub fn find_part(&self, part: Part) -> Option<NodeId> {
    self.ops.find_part(self.node, part)
  }

  /// Registers `controller` for the control's content: the truth it reads is written to
  /// the platform, and a reader's change of the content is answered by it, with the
  /// truth alone while the control is disabled; see [`Controller`](crate::Controller)
  /// and [`EditController`](crate::EditController).
  pub fn register(&self, controller: impl Controls) {
    let disabled = Rc::clone(&self.disabled);

    controller
      .derive_disabled(move || disabled())
      .register_with(self.ops.content(), self.node);
  }

  /// Re-reads what the behavior reports: its state, accessibility and focus order.
  ///
  /// For a change no signal carries; a report that reads signals re-applies on its own.
  #[inline(always)]
  pub fn refresh(&self) {
    self.changed.notify();
  }
}

/// A control's caption: a view mounted as its [`Part::Label`], with the accessible
/// name it supplies when its text is known at build.
#[derive(Debug)]
#[must_use]
pub struct Label<P: Realization> {
  view: AnyView<P>,
  name: Option<Source<String>>,
}

impl<P: Realization> Label<P> {
  /// A caption drawn by `view`; the name is the platform's to read from the part.
  pub fn new(view: impl View<P>) -> Self {
    Self {
      view: view.into_any(),
      name: None,
    }
  }

  /// Sets the accessible name the caption supplies, where its text is not the
  /// platform's to read: what type-ahead matches, and what is reported to assistive technology.
  pub fn set_name(mut self, name: impl Into<String>) -> Self {
    self.name = Some(Source::Fixed(name.into()));

    self
  }

  /// Derives the accessible name the caption supplies from the signals `name` reads.
  pub fn derive_name(mut self, name: impl Fn() -> String + 'static) -> Self {
    self.name = Some(Source::derive(name));

    self
  }

  /// The caption's view and the accessible name it supplies: a view built once, as
  /// `Label::new(node)` with the name, captions each form of a control with two.
  #[inline(always)]
  pub fn into_parts(self) -> (AnyView<P>, Option<Source<String>>) {
    (self.view, self.name)
  }
}

impl<P: Realization> From<String> for Label<P> {
  #[inline(always)]
  fn from(text: String) -> Self {
    Self {
      view: Text::new(text.clone()).into_any(),
      name: Some(Source::Fixed(text)),
    }
  }
}

impl<P: Realization> From<&str> for Label<P> {
  fn from(text: &str) -> Self {
    Self::from(text.to_owned())
  }
}

/// A control: focus, events, decoration, a label, parts and accessibility once, over
/// the [`Behavior`] that says what the interaction is.
///
/// The behavior runs first on every event it acts on and its verdict is the whole
/// verdict; every `handle_` here observes. The control is controlled: it reports, and
/// the signal its behavior is bound to is the truth. Its parts and its child resolve
/// the control-scoped conditions (`Checked`, `Disabled`, …) and their focus against it.
/// Its drawn body, which [`set_child`](Self::set_child) or
/// [`set_child_builder`](Self::set_child_builder) supplies, is optional: a control with
/// a label or parts alone builds without one. Its parts, its body and its label
/// sit in one row half an em apart, centered on the cross axis, the label where the
/// behavior places it ([`Behavior::label_placement`]): leading or trailing in the row,
/// or beside the box, before it among its siblings.
#[must_use]
pub struct Control<P: Realization, B: Behavior> {
  behavior: B,
  members: Members<P>,
}

/// Everything a control carries beside its behavior.
struct Members<P: Realization> {
  inner: LayoutBox<P, PlainChild>,
  label: Option<Label<P>>,
  accessible_name: Option<Source<String>>,
  disabled: Option<Source<bool>>,
  required: Option<Source<bool>>,
  read_only: Option<Source<bool>>,
  invalid: Option<Source<bool>>,
  described_by: Option<NodeId>,
  labelled_by: Option<NodeId>,
  focus_order: Option<Source<FocusOrder>>,
  autofocus: bool,
  shortcut: Option<Shortcut>,
  parts: Vec<(Part, AnyView<P>)>,
  observers: Vec<Handler>,
  child: Option<AnyView<P>>,
}

impl<P: Realization, B: Behavior> std::fmt::Debug for Control<P, B> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Control").finish_non_exhaustive()
  }
}

impl<P: Realization, B: Behavior> Control<P, B> {
  /// A control driven by `behavior`, undecorated and without a body.
  #[inline(always)]
  pub fn new(behavior: B) -> Self {
    let mut inner = LayoutBox::new(Layout::Flex {
      axis: Axis::Horizontal,
      reversed: false,
    });

    inner.style_mut().item_alignment = Some(ItemAlignment::Center.into());
    inner.style_mut().spacing = Some(Expr::font_size(0.5).into());

    Self {
      behavior,
      members: Members {
        inner,
        label: None,
        disabled: None,
        required: None,
        read_only: None,
        invalid: None,
        described_by: None,
        labelled_by: None,
        accessible_name: None,
        focus_order: None,
        autofocus: false,
        shortcut: None,
        parts: Vec::new(),
        observers: Vec::new(),
        child: None,
      },
    }
  }
}

impl<P: Realization, B: Behavior + Default> Default for Control<P, B> {
  #[inline(always)]
  fn default() -> Self {
    Self::new(B::default())
  }
}

impl<P: Realization, B: Behavior> Control<P, B> {
  crate::properties! { box_style_mut, P;
    set_spacing / derive_spacing (spacing) => spacing: Spacing<P::Scalar>,
      "the spacing between the control's parts, body and label; half an em unless told otherwise";
    set_item_alignment / derive_item_alignment (alignment) => item_alignment: ItemAlignment,
      "where the control's parts, body and label sit on the cross axis; centered unless told otherwise";
  }

  crate::forward! { members.inner;
    /// Binds `handle` to the control once built: an overlay is placed against it, focus
    /// moves to it and its measured size is reported through the handle.
    fn bind_handle(handle: crate::NodeHandle<P>);
    /// Sets where the body sits when it fills neither axis.
    fn set_alignment(alignment: impl Into<crate::Value<crate::Alignment, crate::Condition<P::Scalar>>>);
    /// Derives where the body sits from the signals `alignment` reads.
    fn derive_alignment[R: Into<crate::StaticLeaf<crate::Alignment>>](alignment: impl Fn() -> R + 'static);
    /// Names the control for container queries against its size.
    fn set_container_name(name: crate::ContainerName);
  }

  /// Sets the axis the control's parts, body and label run along; horizontal unless
  /// told otherwise.
  #[inline(always)]
  pub fn set_axis(mut self, axis: Axis) -> Self {
    self.members.inner = self.members.inner.set_layout(Layout::Flex {
      axis,
      reversed: false,
    });

    self
  }

  /// Sets the caption: a [`Part::Label`] child whose text is the accessible name.
  ///
  /// A string builds a [`Text`] and names the control; [`Label::new`] mounts any view
  /// and leaves the name to the platform.
  pub fn set_label(mut self, label: impl Into<Label<P>>) -> Self {
    self.members.label = Some(label.into());

    self
  }

  /// Sets the name the control reports to assistive technology, over its caption's: for
  /// a control no caption names, such as a button drawn by an icon alone.
  pub fn set_accessible_name(mut self, name: impl Into<String>) -> Self {
    self.members.accessible_name = Some(Source::Fixed(name.into()));

    self
  }

  /// Derives the name the control reports to assistive technology from the signals
  /// `name` reads.
  pub fn derive_accessible_name(mut self, name: impl Fn() -> String + 'static) -> Self {
    self.members.accessible_name = Some(Source::derive(name));

    self
  }

  /// Sets whether the control is present but not operable: a disabled control takes
  /// no focus, ignores every event it would act on and reports
  /// [`AccessProp::Disabled`].
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.members.disabled = Some(Source::Fixed(disabled));

    self
  }

  /// Derives whether the control is disabled from the signals `disabled` reads.
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.members.disabled = Some(Source::derive(disabled));

    self
  }

  /// Sets whether a value must be supplied before the form the control is in submits.
  #[inline(always)]
  pub fn set_required(mut self, required: bool) -> Self {
    self.members.required = Some(Source::Fixed(required));

    self
  }

  /// Derives whether the control is required from the signals `required` reads.
  pub fn derive_required(mut self, required: impl Fn() -> bool + 'static) -> Self {
    self.members.required = Some(Source::derive(required));

    self
  }

  /// Sets whether the control's value is shown but not editable.
  #[inline(always)]
  pub fn set_read_only(mut self, read_only: bool) -> Self {
    self.members.read_only = Some(Source::Fixed(read_only));

    self
  }

  /// Derives whether the control is read-only from the signals `read_only` reads.
  pub fn derive_read_only(mut self, read_only: impl Fn() -> bool + 'static) -> Self {
    self.members.read_only = Some(Source::derive(read_only));

    self
  }

  /// Sets whether the control holds an invalid value; the `Invalid` state inside it
  /// follows.
  #[inline(always)]
  pub fn set_invalid(mut self, invalid: bool) -> Self {
    self.members.invalid = Some(Source::Fixed(invalid));

    self
  }

  /// Derives whether the control is invalid from the signals `invalid` reads.
  pub fn derive_invalid(mut self, invalid: impl Fn() -> bool + 'static) -> Self {
    self.members.invalid = Some(Source::derive(invalid));

    self
  }

  /// Sets the node whose text describes the control, for assistive technology.
  #[inline(always)]
  pub const fn set_described_by(mut self, node: NodeId) -> Self {
    self.members.described_by = Some(node);

    self
  }

  /// Names the control by the text of `node`, a caption placed anywhere, after the
  /// caption [`set_label`](Self::set_label) builds, if any.
  #[inline(always)]
  pub const fn set_labelled_by(mut self, node: NodeId) -> Self {
    self.members.labelled_by = Some(node);

    self
  }

  /// Sets the control's place in focus traversal, over the behavior's default.
  #[inline(always)]
  pub fn set_focus_order(mut self, focus_order: FocusOrder) -> Self {
    self.members.focus_order = Some(Source::Fixed(focus_order));

    self
  }

  /// Derives the control's place in focus traversal from the signals `focus_order`
  /// reads, over the behavior's default.
  pub fn derive_focus_order(mut self, focus_order: impl Fn() -> FocusOrder + 'static) -> Self {
    self.members.focus_order = Some(Source::derive(focus_order));

    self
  }

  /// Sets whether the control takes focus as its window opens, where otherwise nothing
  /// is focused; the first in the scene's child order of the controls built with the
  /// window that ask for it, and enabled, takes it, and focus does not show for it.
  #[inline(always)]
  pub const fn set_autofocus(mut self, autofocus: bool) -> Self {
    self.members.autofocus = autofocus;

    self
  }

  /// Sets the key that activates the control from anywhere in the window, as a click
  /// does, and that the platform reports beside its name.
  ///
  /// It acts while the control is enabled and shown and no open modal it lies outside
  /// excludes it; of several controls and rows taking one key, the first in the scene's
  /// child order acts. A key holding no modifier but `Shift` does not act while focus is
  /// in a text entry, whose key it is, and a view-scoped binding (a
  /// [`Keymap`](crate::Keymap)'s, a menu entry's) of the same key acts first inside its
  /// view.
  #[inline(always)]
  pub const fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.members.shortcut = Some(shortcut);

    self
  }

  /// Adds `view` as the part `part` of the control, a styling target its conditions
  /// resolve against the control.
  pub fn set_part(mut self, part: Part, view: impl View<P>) -> Self {
    self.members.parts.push((part, view.into_any()));

    self
  }

  /// Sets the drawn body.
  pub fn set_child(mut self, child: impl View<P>) -> Self {
    self.members.child = Some(child.into_any());

    self
  }

  /// Sets the drawn body, built by `child` under the control.
  pub fn set_child_builder<V: View<P>>(mut self, child: impl FnOnce() -> V + 'static) -> Self {
    self.members.child = Some(AnyView::new(move || child().build()));

    self
  }

  /// Runs `action` after the behavior has acted on an activation: a click, or a key the
  /// behavior activates on.
  pub fn handle_activate(mut self, action: impl Fn() + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::Click(Rc::new(move |_: &ClickEvent| {
        action();

        Prevent::None
      })));

    self
  }

  /// Runs `action` after the behavior has acted on a key pressed while the control
  /// holds focus.
  pub fn handle_key_down(mut self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::KeyDown(observe(action)));

    self
  }

  /// Runs `action` on a key released while the control holds focus.
  pub fn handle_key_up(mut self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.members.observers.push(Handler::KeyUp(observe(action)));

    self
  }

  /// Runs `action` after the behavior has acted on a pointer button pressed over the
  /// control.
  pub fn handle_pointer_down(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::PointerDown(observe(action)));

    self
  }

  /// Runs `action` after the behavior has acted on a pointer button released over the
  /// control.
  pub fn handle_pointer_up(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::PointerUp(observe(action)));

    self
  }

  /// Runs `action` after the behavior has acted on the pointer moving over the control.
  pub fn handle_pointer_move(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::PointerMove(observe(action)));

    self
  }

  /// Runs `action` after the behavior has acted on a wheel turned over the control,
  /// before the platform scrolls by it.
  pub fn handle_wheel(mut self, action: impl Fn(&WheelEvent) + 'static) -> Self {
    self.members.observers.push(Handler::Wheel(observe(action)));

    self
  }

  /// Runs `action` after the behavior has acted on the platform taking the pointer
  /// pressed over the control for itself, ending the press without a release.
  pub fn handle_pointer_cancel(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::PointerCancel(observe(action)));

    self
  }

  /// Runs `action` when the pointer enters the control.
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::PointerEnter(Rc::new(action)));

    self
  }

  /// Runs `action` when the pointer leaves the control.
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::PointerLeave(Rc::new(action)));

    self
  }

  /// Runs `action` when focus enters the control or a node inside it from outside.
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.members.observers.push(Handler::Focus(Rc::new(action)));

    self
  }

  /// Runs `action` when focus leaves the control and every node inside it.
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.members.observers.push(Handler::Blur(Rc::new(action)));

    self
  }

  /// Runs `action` with the control's new offsets when it scrolls.
  pub fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self
      .members
      .observers
      .push(Handler::Scroll(Rc::new(action)));

    self
  }
}

impl<P: Realization, B: Behavior> Control<P, B> {
  /// The behavior driving the control.
  #[inline(always)]
  pub const fn behavior(&self) -> &B {
    &self.behavior
  }

  /// This control, its behavior replaced by what `map` makes of it.
  #[inline(always)]
  pub fn map_behavior(mut self, map: impl FnOnce(B) -> B) -> Self {
    self.behavior = map(self.behavior);

    self
  }

  /// This control over `defaults`, as if its setters had been called after the
  /// defaults': its behavior, and everything it sets in place of the default.
  ///
  /// Its box goes over the defaults' as [`LayoutBox::over`] says, its caption, name,
  /// flags, relations, focus order and shortcut replace theirs where it has them, it
  /// asks for focus where either does, its handlers run after theirs, and its body
  /// replaces theirs where it has one. Each of its parts takes the place of the
  /// defaults' part of the same kind, and the rest follow the defaults' in order. A
  /// styled built-in builds its defaults so, under every setter its user called.
  pub fn over<D: Behavior>(self, defaults: Control<P, D>) -> Self {
    let Members {
      inner,
      label,
      accessible_name,
      disabled,
      required,
      read_only,
      invalid,
      described_by,
      labelled_by,
      focus_order,
      autofocus,
      shortcut,
      parts: mut own,
      observers,
      child,
    } = self.members;
    let defaults = defaults.members;
    let mut parts = Vec::with_capacity(defaults.parts.len() + own.len());
    let mut handlers = defaults.observers;

    for (part, view) in defaults.parts {
      match own.iter().position(|(mine, _)| *mine == part) {
        Some(index) => parts.push(own.remove(index)),
        None => parts.push((part, view)),
      }
    }

    parts.extend(own);
    handlers.extend(observers);

    Self {
      behavior: self.behavior,
      members: Members {
        inner: inner.over(defaults.inner),
        label: label.or(defaults.label),
        accessible_name: accessible_name.or(defaults.accessible_name),
        disabled: disabled.or(defaults.disabled),
        required: required.or(defaults.required),
        read_only: read_only.or(defaults.read_only),
        invalid: invalid.or(defaults.invalid),
        described_by: described_by.or(defaults.described_by),
        labelled_by: labelled_by.or(defaults.labelled_by),
        focus_order: focus_order.or(defaults.focus_order),
        autofocus: autofocus || defaults.autofocus,
        shortcut: shortcut.or(defaults.shortcut),
        parts,
        observers: handlers,
        child: child.or(defaults.child),
      },
    }
  }
}

impl<P: Realization, B: Behavior> AnyKind for Control<P, B> {}

impl<P: Realization, B: Behavior> Thin<P> for Control<P, B> {
  type Core = ControlCore<P>;

  #[inline(always)]
  fn into_core(self) -> ControlCore<P> {
    ControlCore {
      behavior: Rc::new(self.behavior),
      members: self.members,
    }
  }
}

impl<P: Realization, B: Behavior> View<P> for Control<P, B> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.into_core().build()
  }
}

/// A [`Control`] with its behavior's type erased: what every control builds, whatever
/// its behavior, the control thin over it.
#[must_use]
pub struct ControlCore<P: Realization> {
  behavior: Rc<dyn Behavior>,
  members: Members<P>,
}

impl<P: Realization> std::fmt::Debug for ControlCore<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ControlCore")
      .finish_non_exhaustive()
  }
}

impl<P: Realization> AnyKind for ControlCore<P> {}

impl<P: Realization> View<P> for ControlCore<P> {
  fn build(self) -> NodeId {
    let Self {
      behavior,
      members:
        Members {
          mut inner,
          label,
          disabled,
          required,
          read_only,
          invalid,
          described_by,
          labelled_by: named_by,
          accessible_name,
          focus_order,
          autofocus,
          shortcut,
          parts,
          observers,
          child,
        },
    } = self;
    let owner = Owner::current().child();

    owner.enter(move || {
      let native = behavior.native();
      let lines = behavior.lines();
      let placement = behavior.label_placement();
      let named = label.as_ref().and_then(|label| label.name.clone());
      let name = named
        .clone()
        .or_else(|| match &accessible_name {
          Some(Source::Fixed(name)) => Some(Source::Fixed(name.clone())),
          Some(Source::Derived(_)) | None => None,
        })
        .map(Source::into_read);

      if let Some(role) = behavior.role() {
        inner = inner.set_role(role);
      }

      if let Some(axes) = behavior.drag_axes() {
        inner = inner.set_drag_axes(axes);
      }

      let node = inner.realize(
        |boxed| {
          NodeFacts::Control(ControlFacts {
            boxed,
            native,
            label: name.as_ref().map(|name| untrack(|| name())),
            lines,
            label_beside: placement == LabelPlacement::Beside,
          })
        },
        |_, _, _| {},
      );

      note_facts::<P>(node, native, placement, autofocus, shortcut);
      // A caption whose text is the platform's to read names the control by reference;
      // it is built after the control, so the markup keeps the control first.
      let (label, labelled_by) = match label {
        Some(Label { view, name: None }) => {
          let part = view.build();

          (Some((part, None)), Some(part))
        }
        Some(Label { view, name }) => (Some((view.build(), name)), None),
        None => (None, None),
      };
      let disabled = read_bool(disabled);
      let handle = Rc::new(ControlHandle {
        node,
        name,
        disabled: Rc::clone(&disabled),
        changed: Trigger::new(),
        ops: const { ControlOps::of::<P>() },
      });
      let flags = Flags {
        disabled: Rc::clone(&disabled),
        required: read_bool(required),
        read_only: read_bool(read_only),
        invalid: read_bool(invalid),
        described_by,
        labelled_by: labelled_by.into_iter().chain(named_by).collect(),
        accessible_name: accessible_name.map(Source::into_read).or(match named {
          Some(Source::Derived(name)) => Some(name),
          Some(Source::Fixed(_)) | None => None,
        }),
        shortcut,
      };

      bind_reports::<P>(&handle, &behavior, &flags, focus_order);
      behavior.attach(&handle);
      register::<P>(node, &behavior, &handle, &disabled, observers);
      mount_contents::<P>(
        node,
        placement,
        label.map(|(part, _)| part),
        parts,
        child.map(View::build),
      );
      behavior.mounted(&handle);

      node
    })
  }
}

/// Records in the scene what the control `node` is beside its node: a text entry where
/// `native` is one, a caption kept beside it where `placement` places it there, a
/// request for focus as the window opens where it makes one, and its shortcut where it
/// has one.
fn note_facts<P: Realization>(
  node: NodeId,
  native: Option<NativeElement>,
  placement: LabelPlacement,
  autofocus: bool,
  shortcut: Option<Shortcut>,
) {
  Scene::<P>::with(|scene| {
    if native.is_some_and(NativeElement::is_text_entry) {
      scene.note_entry(node);
    }

    if placement == LabelPlacement::Beside {
      scene.keep_caption_beside(node);
    }

    if autofocus {
      scene.note_autofocus(node);
    }

    if let Some(shortcut) = shortcut {
      scene.note_shortcut(node, shortcut);
    }
  });
}

/// Mounts what a control holds under `node`, in order: the label where it leads, the
/// parts, the body, then the label where it trails or sits beside.
fn mount_contents<P: Realization>(
  node: NodeId,
  placement: LabelPlacement,
  label: Option<NodeId>,
  parts: Vec<(Part, AnyView<P>)>,
  body: Option<NodeId>,
) {
  let mut label = label;

  if placement == LabelPlacement::Leading
    && let Some(part) = label.take()
  {
    mount_part::<P>(node, Part::Label, part);
  }

  for (part, view) in parts {
    mount_part::<P>(node, part, view.build());
  }

  if let Some(body) = body {
    Scene::<P>::with(|scene| scene.attach(node, body, None));
  }

  if let Some(part) = label {
    mount_part::<P>(node, Part::Label, part);
  }
}

/// A boolean setting as a closure: `false` when unset.
fn read_bool(source: Option<Source<bool>>) -> Rc<dyn Fn() -> bool> {
  source.map_or_else(|| Rc::new(|| false), Source::into_read)
}

/// Attaches `child` under `node` as its `part`.
fn mount_part<P: Realization>(node: NodeId, part: Part, child: NodeId) {
  Scene::<P>::with(|scene| {
    scene.set_part(child, part);
    scene.attach(node, child, None);
  });
}

/// What the control's user set beside the behavior: the flags every control may carry,
/// the node describing it and the name it reports.
#[derive(Clone)]
struct Flags {
  /// Whether the control is disabled, read where it is reported.
  disabled: Rc<dyn Fn() -> bool>,
  /// Whether the control needs a value, read where it is reported.
  required: Rc<dyn Fn() -> bool>,
  /// Whether the control's value is read-only, read where it is reported.
  read_only: Rc<dyn Fn() -> bool>,
  /// Whether the control's value is invalid, read where it is reported.
  invalid: Rc<dyn Fn() -> bool>,
  /// The node describing the control.
  described_by: Option<NodeId>,
  /// The nodes naming the control by reference: its caption, where its text is not
  /// known at build, then the node set to name it.
  labelled_by: Vec<NodeId>,
  /// The name the control reports, read where it is reported: the one set over its
  /// caption's, else its caption's where that is derived.
  accessible_name: Option<Rc<dyn Fn() -> String>>,
  /// The key that activates the control from anywhere in the window.
  shortcut: Option<Shortcut>,
}

/// Binds the render effects writing the control's state, accessibility and focus
/// order, each re-applied when what it reads changes or the handle is refreshed.
fn bind_reports<P: Realization>(
  handle: &Rc<ControlHandle>,
  behavior: &Rc<dyn Behavior>,
  flags: &Flags,
  focus_order: Option<Source<FocusOrder>>,
) {
  let node = handle.node;
  let changed = handle.changed;

  {
    let behavior = Rc::clone(behavior);
    let disabled = Rc::clone(&flags.disabled);
    let invalid = Rc::clone(&flags.invalid);

    Effect::render(move |_: Option<()>| {
      changed.track();

      let mut state = behavior.state();

      state.disabled |= disabled();
      state.invalid |= invalid();
      Scene::<P>::with(|scene| scene.set_control_state(node, state));
    });
  }

  {
    let behavior = Rc::clone(behavior);
    let flags = flags.clone();

    Effect::render(move |_: Option<()>| {
      changed.track();

      let mut access = behavior.access();

      if (flags.disabled)() {
        access.push(AccessProp::Disabled(true));
      }

      if (flags.required)() {
        access.push(AccessProp::Required(true));
      }

      if (flags.read_only)() {
        access.push(AccessProp::ReadOnly(true));
      }

      if (flags.invalid)() {
        access.push(AccessProp::Invalid(true));
      }

      if !flags.labelled_by.is_empty() {
        access.push(AccessProp::LabelledBy(flags.labelled_by.clone()));
      }

      if let Some(described_by) = flags.described_by {
        access.push(AccessProp::DescribedBy(vec![described_by]));
      }

      if let Some(name) = &flags.accessible_name {
        access.push(AccessProp::Name(name()));
      }

      if let Some(shortcut) = flags.shortcut {
        access.push(AccessProp::Shortcut(shortcut));
      }

      report_access::<P>(node, access);
    });
  }

  {
    let behavior = Rc::clone(behavior);
    let disabled = Rc::clone(&flags.disabled);
    let chosen: Option<Rc<dyn Fn() -> FocusOrder>> = match focus_order {
      None => None,
      Some(Source::Fixed(order)) => Some(Rc::new(move || order)),
      Some(Source::Derived(compute)) => Some(compute),
    };

    Effect::render(move |_: Option<()>| {
      changed.track();

      let order = if disabled() {
        None
      }
      else if let Some(chosen) = &chosen {
        Some(chosen())
      }
      else {
        behavior.focus_order()
      };

      Scene::<P>::with(|scene| scene.set_focus_order(node, order));
    });
  }
}

/// What every acting handler shares: the behavior, its handle and the disabled read.
struct Actor {
  behavior: Rc<dyn Behavior>,
  handle: Rc<ControlHandle>,
  disabled: Rc<dyn Fn() -> bool>,
}

impl Clone for Actor {
  fn clone(&self) -> Self {
    Self {
      behavior: Rc::clone(&self.behavior),
      handle: Rc::clone(&self.handle),
      disabled: Rc::clone(&self.disabled),
    }
  }
}

impl Actor {
  /// The behavior's handler for an event of `kind`, `None` for a kind no behavior
  /// acts on: a disabled control answers without acting, and a key an input method is
  /// composing never reaches the behavior.
  fn acting(&self, kind: EventKind) -> Option<Handler> {
    let pointer = |phase: PointerPhase| {
      self.act(kind, move |behavior, event: &PointerEvent, handle| {
        behavior.on_pointer(phase, event, handle)
      })
    };

    Some(match kind {
      EventKind::Click => {
        Handler::ActingClick(self.act(kind, |behavior, _: &ClickEvent, handle| {
          behavior.on_click(handle)
        }))
      }
      EventKind::KeyDown => {
        Handler::KeyDown(self.act(kind, |behavior, event: &KeyEvent, handle| {
          if event.composing {
            Prevent::None
          }
          else {
            behavior.on_key(event, handle)
          }
        }))
      }
      EventKind::KeyUp => Handler::KeyUp(self.act(kind, |behavior, event: &KeyEvent, handle| {
        if event.composing {
          Prevent::None
        }
        else {
          behavior.on_key_up(event, handle)
        }
      })),
      EventKind::PointerDown => Handler::PointerDown(pointer(PointerPhase::Down)),
      EventKind::PointerUp => Handler::PointerUp(pointer(PointerPhase::Up)),
      EventKind::PointerMove => Handler::PointerMove(pointer(PointerPhase::Move)),
      EventKind::PointerCancel => Handler::PointerCancel(pointer(PointerPhase::Cancel)),
      EventKind::Wheel => Handler::Wheel(self.act(kind, |behavior, event: &WheelEvent, handle| {
        behavior.on_wheel(event, handle)
      })),
      EventKind::Blur => {
        let actor = self.clone();

        Handler::Blur(Rc::new(move || actor.behavior.on_blur(&actor.handle)))
      }
      _ => return None,
    })
  }

  /// `observer` as the control registers it: the events a disabled control answers
  /// for, it answers without running `observer`.
  fn gate(&self, observer: Handler) -> Handler {
    match observer {
      Handler::Click(run) => Handler::Click(self.gated(EventKind::Click, run)),
      Handler::KeyDown(run) => Handler::KeyDown(self.gated(EventKind::KeyDown, run)),
      Handler::KeyUp(run) => Handler::KeyUp(self.gated(EventKind::KeyUp, run)),
      Handler::PointerDown(run) => Handler::PointerDown(self.gated(EventKind::PointerDown, run)),
      Handler::PointerUp(run) => Handler::PointerUp(self.gated(EventKind::PointerUp, run)),
      Handler::PointerMove(run) => Handler::PointerMove(self.gated(EventKind::PointerMove, run)),
      Handler::PointerCancel(run) => {
        Handler::PointerCancel(self.gated(EventKind::PointerCancel, run))
      }
      Handler::Wheel(run) => Handler::Wheel(self.gated(EventKind::Wheel, run)),
      observer => observer,
    }
  }

  /// `run` for an event of `kind`, answered as a disabled control answers it while
  /// disabled.
  fn gated<E: ?Sized + 'static>(&self, kind: EventKind, run: Rc<Acting<E>>) -> Rc<Acting<E>> {
    self.act(kind, move |_, event, _| run(event))
  }

  /// A handler for an event of `kind` running `act`, answered as a disabled control
  /// answers it while disabled: every pointer event and click swallowed, every key and
  /// wheel let by.
  fn act<E: ?Sized + 'static>(
    &self,
    kind: EventKind,
    act: impl Fn(&dyn Behavior, &E, &ControlHandle) -> Prevent + 'static,
  ) -> Rc<Acting<E>> {
    let actor = self.clone();
    let when_disabled = match kind {
      EventKind::Click
      | EventKind::PointerDown
      | EventKind::PointerUp
      | EventKind::PointerMove
      | EventKind::PointerCancel => Prevent::All,
      _ => Prevent::None,
    };

    Rc::new(move |event| {
      if (actor.disabled)() {
        when_disabled
      }
      else {
        act(&*actor.behavior, event, &actor.handle)
      }
    })
  }
}

/// Registers the behavior's handler for each event it acts on, then the observers:
/// the behavior acts first and its verdict stands, and the observers see what it did.
fn register<P: Realization>(
  node: NodeId,
  behavior: &Rc<dyn Behavior>,
  handle: &Rc<ControlHandle>,
  disabled: &Rc<dyn Fn() -> bool>,
  observers: Vec<Handler>,
) {
  let actor = Actor {
    behavior: Rc::clone(behavior),
    handle: Rc::clone(handle),
    disabled: Rc::clone(disabled),
  };
  let acting: Vec<Handler> = behavior
    .listens()
    .iter()
    .filter_map(|kind| actor.acting(*kind))
    .collect();

  Scene::<P>::with(|scene| {
    for handler in acting {
      scene.handle(node, handler);
    }

    for observer in observers {
      scene.handle(node, actor.gate(observer));
    }
  });
}

/// `action` as an observing handler, leaving the verdict to the behavior.
fn observe<E: ?Sized + 'static>(action: impl Fn(&E) + 'static) -> Rc<Acting<E>> {
  Rc::new(move |event| {
    action(event);

    Prevent::None
  })
}

/// A button: [`Control`] over [`Activate`].
pub type BasicButton<P> = Control<P, Activate>;
/// A checkbox: [`Control`] over [`Toggle`].
pub type BasicCheckbox<P> = Control<P, Toggle>;
/// A slider: [`Control`] over [`Slider`].
pub type BasicSlider<P> = Control<P, Slider>;
/// One radio choice: [`Control`] over [`Radio`].
pub type BasicRadio<P> = Control<P, Radio>;
/// A single-line text field: [`Control`] over [`TextEntry`].
pub type BasicTextField<P> = Control<P, TextEntry>;
/// A multi-line text editor: [`Control`] over [`TextEntry`] with
/// [`set_lines`](TextEntry::set_lines) above one.
pub type BasicTextEditor<P> = Control<P, TextEntry>;
/// One option within a listbox: [`Control`] over [`ListboxOption`].
pub type BasicListboxOption<P> = Control<P, ListboxOption>;
/// A menu: [`Control`] over [`Menu`].
pub type BasicMenu<P> = Control<P, Menu>;
/// One item within a menu: [`Control`] over [`MenuOption`].
pub type BasicMenuItem<P> = Control<P, MenuOption>;

/// What a facade's radio group carries beyond every control's members: what it has
/// chosen, a value of the app's own type `T`, its radios tagged with `T`s.
///
/// The group is a [`Control`] over a [`RadioGroup`], which holds the choice erased, so
/// every group is one type whatever its `T`. A group that starts with nothing chosen
/// holds an `Option`, its radios tagged `Some(..)`.
pub trait BasicRadioGroupBase<P: Realization, T: Choice>: HasControl<P, RadioGroup> {
  /// Sets the chosen radio's value.
  #[must_use]
  fn set_chosen(self, chosen: T) -> Self {
    self.map_control(|control| control.map_behavior(|group| group.set_chosen(Some(chosen))))
  }

  /// Derives the chosen radio's value from the signals `chosen` reads.
  #[must_use]
  fn derive_chosen(self, chosen: impl Fn() -> T + 'static) -> Self {
    self.map_control(|control| {
      control.map_behavior(|group| group.derive_chosen(move || Some(chosen())))
    })
  }

  /// Runs `action` with the value a choice asks for.
  #[must_use]
  fn handle_choose(self, action: impl Fn(T) + 'static) -> Self {
    self.map_control(|control| {
      control.map_behavior(|group| {
        group.handle_choose(move |chosen: Option<T>| {
          if let Some(chosen) = chosen {
            action(chosen);
          }
        })
      })
    })
  }

  /// Binds the chosen value to `signal` in both directions.
  #[must_use]
  fn bind_chosen(self, signal: Signal<T>) -> Self {
    self
      .derive_chosen(move || signal.get())
      .handle_choose(move |chosen| signal.set(chosen))
  }
}

/// What a facade's listbox carries beyond every control's members: what it has chosen,
/// a [`Chosen`] of the app's own type, `Option<T>` for one choice, its options tagged
/// with `T`s.
///
/// The listbox is a [`Control`] over a [`Listbox`], which holds the choice erased, so
/// every listbox is one type whatever its `C`.
pub trait BasicListboxBase<P: Realization, C: Chosen>: HasControl<P, Listbox> {
  /// Sets what is chosen.
  #[must_use]
  fn set_chosen(self, chosen: C) -> Self {
    self.map_control(|control| control.map_behavior(|listbox| listbox.set_chosen(chosen)))
  }

  /// Derives what is chosen from the signals `chosen` reads.
  #[must_use]
  fn derive_chosen(self, chosen: impl Fn() -> C + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|listbox| listbox.derive_chosen(chosen)))
  }

  /// Runs `action` with what a choice asks for.
  #[must_use]
  fn handle_choose(self, action: impl Fn(C) + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|listbox| listbox.handle_choose(action)))
  }

  /// Binds what is chosen to `signal` in both directions.
  #[must_use]
  fn bind_chosen(self, signal: Signal<C>) -> Self {
    self.map_control(|control| control.map_behavior(|listbox| listbox.bind_chosen(signal)))
  }
}

/// What a facade's checkbox and switch carry beyond every control's members: whether
/// they are checked, a switch being on while checked, controlled.
///
/// The view is a [`Control`] over a [`Toggle`].
pub trait ToggleBase<P: Realization>: HasControl<P, Toggle> {
  /// Sets whether the control is checked.
  #[must_use]
  fn set_checked(self, checked: bool) -> Self {
    self.map_control(|control| control.map_behavior(|toggle| toggle.set_checked(checked)))
  }

  /// Derives whether the control is checked from the signals `checked` reads.
  #[must_use]
  fn derive_checked(self, checked: impl Fn() -> bool + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|toggle| toggle.derive_checked(checked)))
  }

  /// Runs `action` with the state a flip asks for.
  #[must_use]
  fn handle_toggle(self, action: impl Fn(bool) + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|toggle| toggle.handle_toggle(action)))
  }

  /// Binds whether the control is checked to `signal` in both directions.
  #[must_use]
  fn bind_checked(self, signal: Signal<bool>) -> Self {
    self.map_control(|control| control.map_behavior(|toggle| toggle.bind_checked(signal)))
  }
}

/// What a facade's slider carries beyond every control's members: its range and its
/// value, controlled, and the width of the thumb the pointer is measured against.
///
/// The view is a [`Control`] over a [`Slider`].
pub trait SliderBase<P: Realization>: HasControl<P, Slider> {
  /// Sets the range: its bounds and the step values snap to.
  #[must_use]
  fn set_range(self, range: SliderRange) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.set_range(range)))
  }

  /// Derives the range from the signals `range` reads.
  #[must_use]
  fn derive_range(self, range: impl Fn() -> SliderRange + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.derive_range(range)))
  }

  /// Sets the value.
  #[must_use]
  fn set_value(self, value: f32) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.set_value(value)))
  }

  /// Derives the value from the signals `value` reads.
  #[must_use]
  fn derive_value(self, value: impl Fn() -> f32 + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.derive_value(value)))
  }

  /// Runs `action` with the value a move asks for.
  #[must_use]
  fn handle_change(self, action: impl Fn(f32) + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.handle_change(action)))
  }

  /// Binds the value to `signal` in both directions.
  #[must_use]
  fn bind_value(self, signal: Signal<f32>) -> Self {
    self.map_control(|control| control.map_behavior(|slider| slider.bind_value(signal)))
  }

  /// Sets the width of the thumb the pointer is measured against: its center travels
  /// from half of it past the track's start to half of it short of its end; the default
  /// thumb's unless told otherwise.
  #[must_use]
  fn set_thumb_extent(self, extent: impl Into<crate::Length>) -> Self {
    let extent = extent.into();

    self.map_control(|control| control.map_behavior(|slider| slider.set_thumb_extent(extent)))
  }
}

/// What a facade's text field and text editor carry beyond every control's members:
/// the text and the selection, both controlled, the hint, the keyboard and the colors
/// the value is selected in.
///
/// The entry is a [`Control`] over a [`TextEntry`]; the colors are its
/// [`SelectionColors`], which the view redefines at its node as it builds.
pub trait TextEntryBase<P: Realization>: HasControl<P, TextEntry> {
  /// The colors the value is selected in, which the view defines as it builds.
  fn selection_colors_mut(&mut self) -> &mut SelectionColors<P::Scalar>;

  crate::trait_properties! { selection_colors_mut, Condition<P::Scalar>;
    set_selection_color / derive_selection_color (color) => color: crate::Color,
      "the text color of the selected value",
      "A variant testing the environment takes effect; one testing the entry's state does not.";
    set_selection_background / derive_selection_background (color) => background: crate::Color,
      "the background behind the selected value",
      "A variant testing the environment takes effect; one testing the entry's state does not.";
  }

  /// Sets the text.
  #[must_use]
  fn set_value(self, value: impl Into<String>) -> Self {
    let value = value.into();

    self.map_control(|control| control.map_behavior(|entry| entry.set_value(value)))
  }

  /// Derives the text from the signals `value` reads.
  #[must_use]
  fn derive_value(self, value: impl Fn() -> String + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.derive_value(value)))
  }

  /// Runs `action` with the text an edit asks for.
  #[must_use]
  fn handle_change(self, action: impl Fn(&str) + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.handle_change(action)))
  }

  /// Binds the text to `signal` in both directions.
  #[must_use]
  fn bind_value(self, signal: Signal<String>) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.bind_value(signal)))
  }

  /// Sets the hint shown while the text is empty.
  #[must_use]
  fn set_placeholder(self, placeholder: impl Into<String>) -> Self {
    let placeholder = placeholder.into();

    self.map_control(|control| control.map_behavior(|entry| entry.set_placeholder(placeholder)))
  }

  /// Derives the hint shown while the text is empty from the signals `placeholder`
  /// reads.
  #[must_use]
  fn derive_placeholder(self, placeholder: impl Fn() -> String + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.derive_placeholder(placeholder)))
  }

  /// Sets the keyboard the entry asks the platform for.
  #[must_use]
  fn set_keyboard(self, keyboard: Keyboard) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.set_keyboard(keyboard)))
  }

  /// Derives the keyboard the entry asks the platform for from the signals `keyboard`
  /// reads.
  #[must_use]
  fn derive_keyboard(self, keyboard: impl Fn() -> Keyboard + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.derive_keyboard(keyboard)))
  }

  /// Sets the selection, in grapheme clusters of the text.
  #[must_use]
  fn set_selection(self, selection: TextSelection) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.set_selection(selection)))
  }

  /// Derives the selection from the signals `selection` reads.
  #[must_use]
  fn derive_selection(self, selection: impl Fn() -> TextSelection + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.derive_selection(selection)))
  }

  /// Runs `action` with the selection after every move of the caret or its anchor.
  #[must_use]
  fn handle_selection_change(self, action: impl Fn(TextSelection) + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.handle_selection_change(action)))
  }

  /// Binds the selection to `signal` in both directions.
  #[must_use]
  fn bind_selection(self, signal: Signal<TextSelection>) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.bind_selection(signal)))
  }
}

impl<P: Realization, B: Behavior> HasBoxStyle<P> for Control<P, B> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.members.inner.style_mut()
  }
}

impl<P: Realization, B: Behavior> HasLayoutBox<P> for Control<P, B> {
  type Kind = PlainChild;

  #[inline(always)]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<P, PlainChild>) -> LayoutBox<P, PlainChild>,
  ) -> Self {
    let Self {
      behavior,
      mut members,
    } = self;

    members.inner = map(members.inner);

    Self { behavior, members }
  }
}

impl<P: Realization, B: Behavior> DecorationBase<P> for Control<P, B> {}

impl<P: Realization, B: Behavior> MotionBase<P> for Control<P, B> {}

/// A view over a [`Control`], which the members every control carries reach through.
///
/// The control itself, a facade's view holding one as its whole content, and a styled
/// built-in holding one beside what it draws by default.
pub trait HasControl<P: Realization, B: Behavior>: Sized {
  /// The control.
  fn control(&self) -> &Control<P, B>;

  /// The view with its control replaced by what `map` makes of it.
  #[must_use]
  fn map_control(self, map: impl FnOnce(Control<P, B>) -> Control<P, B>) -> Self;
}

impl<P: Realization, B: Behavior> HasControl<P, B> for Control<P, B> {
  #[inline(always)]
  fn control(&self) -> &Self {
    self
  }

  #[inline(always)]
  fn map_control(self, map: impl FnOnce(Self) -> Self) -> Self {
    map(self)
  }
}

impl<P: Realization, B: Behavior, T: Wraps<Inner: HasControl<P, B>>> HasControl<P, B> for T {
  #[inline(always)]
  fn control(&self) -> &Control<P, B> {
    self.inner().control()
  }

  #[inline(always)]
  fn map_control(self, map: impl FnOnce(Control<P, B>) -> Control<P, B>) -> Self {
    Self::wrap(self.into_inner().map_control(map))
  }
}

/// What a facade's [`Control`] carries beyond its decoration and motion.
///
/// Its constructor, its row, its caption, its reported state, its parts and its handlers.
pub trait ControlBase<P: Realization, B: Behavior>: HasControl<P, B> {
  /// A control driven by `behavior`, undecorated and without a body.
  #[must_use]
  fn new(behavior: B) -> Self
  where
    Self: Wraps<Inner = Control<P, B>>,
  {
    Self::wrap(Control::new(behavior))
  }

  /// The behavior driving the control.
  fn behavior(&self) -> &B {
    self.control().behavior()
  }

  /// Sets the axis the control's parts, body and label run along; horizontal
  /// unless told otherwise.
  #[must_use]
  fn set_axis(self, axis: Axis) -> Self {
    self.map_control(|control| control.set_axis(axis))
  }

  /// Sets the spacing between the control's parts, body and label; half an em unless told
  /// otherwise.
  #[must_use]
  fn set_spacing(
    self,
    spacing: impl Into<Value<Spacing<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    self.map_control(|control| control.set_spacing(spacing))
  }

  /// Derives the spacing between the control's parts, body and label from the signals
  /// `spacing` reads: a value, a token or an expression.
  #[must_use]
  fn derive_spacing<R: Into<StaticLeaf<Spacing<P::Scalar>>>>(
    self,
    spacing: impl Fn() -> R + 'static,
  ) -> Self {
    self.map_control(|control| control.derive_spacing(spacing))
  }

  /// Sets where the control's parts, body and label sit on the cross axis; centered unless
  /// told otherwise.
  #[must_use]
  fn set_item_alignment(
    self,
    alignment: impl Into<Value<ItemAlignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.map_control(|control| control.set_item_alignment(alignment))
  }

  /// Derives where the control's parts, body and label sit on the cross axis from the
  /// signals `alignment` reads.
  #[must_use]
  fn derive_item_alignment<R: Into<StaticLeaf<ItemAlignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.map_control(|control| control.derive_item_alignment(alignment))
  }

  /// Binds `handle` to the control once built: an overlay is placed against it, focus
  /// moves to it and its measured size is reported through the handle.
  #[must_use]
  fn bind_handle(self, handle: NodeHandle<P>) -> Self {
    self.map_control(|control| control.bind_handle(handle))
  }

  /// Sets where the body sits when it fills neither axis.
  #[must_use]
  fn set_alignment(self, alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>) -> Self {
    self.map_control(|control| control.set_alignment(alignment))
  }

  /// Derives where the body sits from the signals `alignment` reads.
  #[must_use]
  fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.map_control(|control| control.derive_alignment(alignment))
  }

  /// Names the control for container queries against its size.
  #[must_use]
  fn set_container_name(self, name: ContainerName) -> Self {
    self.map_control(|control| control.set_container_name(name))
  }

  /// Sets the caption: a [`Part::Label`](Part::Label) child whose text is the
  /// accessible name.
  ///
  /// A string builds a text and names the control; [`Label::new`](Label::new)
  /// mounts any view and leaves the name to the platform.
  #[must_use]
  fn set_label(self, label: impl Into<Label<P>>) -> Self {
    self.map_control(|control| control.set_label(label))
  }

  /// Sets the name the control reports to assistive technology, over its
  /// caption's: for a control no caption names, such as a button drawn by an icon
  /// alone.
  #[must_use]
  fn set_accessible_name(self, name: impl Into<String>) -> Self {
    self.map_control(|control| control.set_accessible_name(name))
  }

  /// Derives the name the control reports to assistive technology from the signals
  /// `name` reads.
  #[must_use]
  fn derive_accessible_name(self, name: impl Fn() -> String + 'static) -> Self {
    self.map_control(|control| control.derive_accessible_name(name))
  }

  /// Sets whether the control is present but not operable: a disabled control
  /// takes no focus, ignores every event it would act on and reports
  /// [`AccessProp::Disabled`](AccessProp::Disabled).
  #[must_use]
  fn set_disabled(self, disabled: bool) -> Self {
    self.map_control(|control| control.set_disabled(disabled))
  }

  /// Derives whether the control is disabled from the signals `disabled` reads.
  #[must_use]
  fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.map_control(|control| control.derive_disabled(disabled))
  }

  /// Sets whether a value must be supplied before the form the control is in
  /// submits.
  #[must_use]
  fn set_required(self, required: bool) -> Self {
    self.map_control(|control| control.set_required(required))
  }

  /// Derives whether the control is required from the signals `required` reads.
  #[must_use]
  fn derive_required(self, required: impl Fn() -> bool + 'static) -> Self {
    self.map_control(|control| control.derive_required(required))
  }

  /// Sets whether the control's value is shown but not editable.
  #[must_use]
  fn set_read_only(self, read_only: bool) -> Self {
    self.map_control(|control| control.set_read_only(read_only))
  }

  /// Derives whether the control is read-only from the signals `read_only` reads.
  #[must_use]
  fn derive_read_only(self, read_only: impl Fn() -> bool + 'static) -> Self {
    self.map_control(|control| control.derive_read_only(read_only))
  }

  /// Sets whether the control holds an invalid value; the `Invalid` state inside
  /// it follows.
  #[must_use]
  fn set_invalid(self, invalid: bool) -> Self {
    self.map_control(|control| control.set_invalid(invalid))
  }

  /// Derives whether the control is invalid from the signals `invalid` reads.
  #[must_use]
  fn derive_invalid(self, invalid: impl Fn() -> bool + 'static) -> Self {
    self.map_control(|control| control.derive_invalid(invalid))
  }

  /// Sets the node whose text describes the control, for assistive technology.
  #[must_use]
  fn set_described_by(self, node: NodeId) -> Self {
    self.map_control(|control| control.set_described_by(node))
  }

  /// Names the control by the text of `node`, a caption placed anywhere, after the
  /// caption [`set_label`](Self::set_label) builds, if any.
  #[must_use]
  fn set_labelled_by(self, node: NodeId) -> Self {
    self.map_control(|control| control.set_labelled_by(node))
  }

  /// Sets the control's place in focus traversal, over the behavior's default.
  #[must_use]
  fn set_focus_order(self, focus_order: FocusOrder) -> Self {
    self.map_control(|control| control.set_focus_order(focus_order))
  }

  /// Derives the control's place in focus traversal from the signals
  /// `focus_order` reads, over the behavior's default.
  #[must_use]
  fn derive_focus_order(self, focus_order: impl Fn() -> FocusOrder + 'static) -> Self {
    self.map_control(|control| control.derive_focus_order(focus_order))
  }

  /// Sets whether the control takes focus as its window opens, where otherwise
  /// nothing is focused; the first in the scene's child order of the controls built
  /// with the window that ask for it, and enabled, takes it, and focus does not show
  /// for it.
  #[must_use]
  fn set_autofocus(self, autofocus: bool) -> Self {
    self.map_control(|control| control.set_autofocus(autofocus))
  }

  /// Sets the key that activates the control from anywhere in the window, as a click
  /// does, and that the platform reports beside its name.
  ///
  /// It acts while the control is enabled and shown and no open modal it lies outside
  /// excludes it; of several controls and rows taking one key, the first in the
  /// scene's child order acts. A key holding no modifier but `Shift` does not act
  /// while focus is in a text entry, whose key it is, and a view-scoped binding (a
  /// [`Keymap`](crate::Keymap)'s, a menu entry's) of the same key acts first inside
  /// its view.
  #[must_use]
  fn set_shortcut(self, shortcut: Shortcut) -> Self {
    self.map_control(|control| control.set_shortcut(shortcut))
  }

  /// Adds `view` as the part `part` of the control, a styling target its
  /// conditions resolve against the control.
  #[must_use]
  fn set_part(self, part: Part, view: impl View<P>) -> Self {
    self.map_control(|control| control.set_part(part, view))
  }

  /// Sets the drawn body.
  #[must_use]
  fn set_child(self, child: impl View<P>) -> Self {
    self.map_control(|control| control.set_child(child))
  }

  /// Sets the drawn body, built by `child` under the control.
  #[must_use]
  fn set_child_builder<V: View<P>>(self, child: impl FnOnce() -> V + 'static) -> Self {
    self.map_control(|control| control.set_child_builder(child))
  }

  /// Runs `action` after the behavior has acted on an activation: a click, or a key the
  /// behavior activates on.
  #[must_use]
  fn handle_activate(self, action: impl Fn() + 'static) -> Self {
    self.map_control(|control| control.handle_activate(action))
  }

  /// Runs `action` after the behavior has acted on a key pressed while the control
  /// holds focus.
  #[must_use]
  fn handle_key_down(self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_key_down(action))
  }

  /// Runs `action` on a key released while the control holds focus.
  #[must_use]
  fn handle_key_up(self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_key_up(action))
  }

  /// Runs `action` after the behavior has acted on a pointer button pressed over
  /// the control.
  #[must_use]
  fn handle_pointer_down(self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_pointer_down(action))
  }

  /// Runs `action` after the behavior has acted on a pointer button released over
  /// the control.
  #[must_use]
  fn handle_pointer_up(self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_pointer_up(action))
  }

  /// Runs `action` after the behavior has acted on the pointer moving over the
  /// control.
  #[must_use]
  fn handle_pointer_move(self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_pointer_move(action))
  }

  /// Runs `action` after the behavior has acted on a wheel turned over the
  /// control, before the platform scrolls by it.
  #[must_use]
  fn handle_wheel(self, action: impl Fn(&WheelEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_wheel(action))
  }

  /// Runs `action` after the behavior has acted on the platform taking the pointer
  /// pressed over the control for itself, ending the press without a release.
  #[must_use]
  fn handle_pointer_cancel(self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_pointer_cancel(action))
  }

  /// Runs `action` when the pointer enters the control.
  #[must_use]
  fn handle_pointer_enter(self, action: impl Fn() + 'static) -> Self {
    self.map_control(|control| control.handle_pointer_enter(action))
  }

  /// Runs `action` when the pointer leaves the control.
  #[must_use]
  fn handle_pointer_leave(self, action: impl Fn() + 'static) -> Self {
    self.map_control(|control| control.handle_pointer_leave(action))
  }

  /// Runs `action` when focus enters the control or a node inside it from outside.
  #[must_use]
  fn handle_focus(self, action: impl Fn() + 'static) -> Self {
    self.map_control(|control| control.handle_focus(action))
  }

  /// Runs `action` when focus leaves the control and every node inside it.
  #[must_use]
  fn handle_blur(self, action: impl Fn() + 'static) -> Self {
    self.map_control(|control| control.handle_blur(action))
  }

  /// Runs `action` with the control's new offsets when it scrolls.
  #[must_use]
  fn handle_scroll(self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.map_control(|control| control.handle_scroll(action))
  }
}
