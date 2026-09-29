//! The one control builder, the behavior it is given, and the behaviors that ship.

mod activate;
mod listbox;
mod menu;
mod native_picker;
mod popup;
mod range;
mod roster;
mod roving;
mod text_entry;
mod toggle;
mod type_ahead;

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  AccessProp,
  Alignment,
  Animation,
  AnyKind,
  AnyView,
  Axis,
  BoxKeyframe,
  Condition,
  ContainerName,
  ControlKind,
  ControlProps,
  ControlState,
  DeclaredBoxStyle,
  EventKind,
  Expr,
  FlexDirection,
  FocusOrder,
  IntoView,
  ItemAlignment,
  KeyEvent,
  Layout,
  NoChild,
  NodeId,
  NodeKind,
  Part,
  PlainChild,
  PlainHandler,
  Platform,
  PointerEvent,
  PointerHandler,
  Prevent,
  Prevented,
  Role,
  Scene,
  ScrollEvent,
  ScrollHandler,
  Spacing,
  StaticLeaf,
  StyleExtension,
  TextSelection,
  Transition,
  Value,
  View,
  dispatch_click,
};
use skene_reactive::{Effect, Owner, Trigger, untrack};

pub use self::{
  activate::Activate,
  listbox::{ListItem, Listbox, ListboxGroup},
  menu::{Menu, MenuGroup, MenuOption},
  native_picker::NativePicker,
  popup::Popup,
  range::Range,
  roster::Roster,
  roving::{RadioMember, Roving, RovingGroup},
  text_entry::TextEntry,
  toggle::Toggle,
  type_ahead::{TYPE_AHEAD_RESET, TypeAhead},
};
use crate::{
  Anchor,
  FocusHandle,
  LayoutBox,
  Text,
  decoration,
  properties,
  realize::{Source, report_access, with_sink},
};

/// An observer of an event carrying `E`: a handler with no verdict.
type Observer<E> = dyn Fn(&E);
/// A key observer.
type KeyObserver = Observer<KeyEvent>;
/// A pointer observer.
type PointerObserver = Observer<PointerEvent>;
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
/// [`access`]: Self::access
/// [`focus_order`]: Self::focus_order
/// [`state`]: Self::state
/// [`listens`]: Self::listens
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a control behavior",
  label = "a behavior states a control's kind, role, the events it acts on, and what it reports"
)]
pub trait Behavior<P: Platform>: 'static {
  /// What the control is.
  fn kind(&self) -> ControlKind;

  /// The semantic role the control's node carries; `None` where the kind says enough.
  fn role(&self) -> Option<Role>;

  /// The events the behavior acts on.
  fn listens(&self) -> &'static [EventKind];

  /// The hint a text entry shows while its value is empty.
  #[inline(always)]
  fn placeholder(&self) -> Option<String> {
    None
  }

  /// The lines a text entry shows at once.
  #[inline(always)]
  fn lines(&self) -> u16 {
    1
  }

  /// Where the control's label sits: beside the box of a control whose box holds a
  /// value—a text entry, a picker, a popup face—and after its parts and body inside
  /// it otherwise.
  #[inline(always)]
  fn label_placement(&self) -> LabelPlacement {
    match self.kind() {
      ControlKind::TextField
      | ControlKind::TextEditor
      | ControlKind::NativePicker
      | ControlKind::Combobox => LabelPlacement::Beside,
      _ => LabelPlacement::Trailing,
    }
  }

  /// Runs once the control's node exists and its reports have been read, before its
  /// parts and child build, under the control's owner: what a behavior binds to the
  /// node, provides to its subtree or joins goes here. A behavior whose reports change
  /// here calls [`ControlHandle::refresh`].
  #[inline(always)]
  fn attach(&self, _control: &ControlHandle<P>) {}

  /// What the behavior does on a click.
  #[inline(always)]
  fn on_click(&self, _control: &ControlHandle<P>) -> Prevent {
    Prevent::None
  }

  /// What the behavior does on a key pressed while the control holds focus.
  #[inline(always)]
  fn on_key(&self, _event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    Prevent::None
  }

  /// What the behavior does on a key released while the control holds focus.
  #[inline(always)]
  fn on_key_up(&self, _event: &KeyEvent, _control: &ControlHandle<P>) -> Prevent {
    Prevent::None
  }

  /// What the behavior does when the control loses focus.
  #[inline(always)]
  fn on_blur(&self, _control: &ControlHandle<P>) {}

  /// What the behavior does on a pointer transition over the control.
  #[inline(always)]
  fn on_pointer(
    &self,
    _phase: PointerPhase,
    _event: &PointerEvent,
    _control: &ControlHandle<P>,
  ) -> Prevent {
    Prevent::None
  }

  /// What the behavior does when the platform reports a new textual value.
  #[inline(always)]
  fn on_change(&self, _value: &str, _control: &ControlHandle<P>) {}

  /// What the behavior does when the platform reports a new selection.
  #[inline(always)]
  fn on_selection_change(&self, _selection: TextSelection, _control: &ControlHandle<P>) {}

  /// What the behavior does when the platform reports a new checked state.
  #[inline(always)]
  fn on_toggle(&self, _checked: bool, _control: &ControlHandle<P>) {}

  /// Writes what the behavior holds back to the platform after a report of a change
  /// the platform made itself, so the platform shows it whatever the report's handler
  /// did; the control's state and accessibility are re-read beside it.
  #[inline(always)]
  fn reconcile(&self, _control: &ControlHandle<P>) {}

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

/// A behavior's access to the control it drives: the node, its sink, focus, a click
/// dispatched at it, and a re-read of its reports.
#[must_use]
pub struct ControlHandle<P: Platform> {
  node: NodeId,
  name: Option<Rc<dyn Fn() -> String>>,
  disabled: Rc<dyn Fn() -> bool>,
  changed: Trigger,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> std::fmt::Debug for ControlHandle<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ControlHandle")
      .finish_non_exhaustive()
  }
}

impl<P: Platform> ControlHandle<P> {
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

  /// Runs `operation` on a copy of the control's sink, if the node is live; the scene
  /// is released before it runs.
  #[inline(always)]
  pub fn with_sink<R>(&self, operation: impl FnOnce(P::Sink) -> R) -> Option<R> {
    with_sink::<P, _>(self.node, operation)
  }

  /// Moves focus to the control.
  #[inline(always)]
  pub fn focus(&self) {
    self.with_sink(|sink| P::focus(sink));
  }

  /// Dispatches a click at the control, so its behavior and handlers see it.
  #[inline(always)]
  pub fn click(&self) -> Prevented {
    dispatch_click::<P>(self.node)
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
pub struct Label<P: Platform> {
  view: AnyView<P>,
  name: Option<Source<String>>,
}

impl<P: Platform> Label<P> {
  /// A caption drawn by `view`; the name is the platform's to read from the part.
  #[inline(always)]
  pub fn new(view: impl IntoView<P>) -> Self {
    Self {
      view: view.into_view(),
      name: None,
    }
  }

  /// Sets the accessible name the caption supplies, where its text is not the
  /// platform's to read: what type-ahead matches, and what is reported to assistive technology.
  #[inline(always)]
  pub fn set_name(mut self, name: impl Into<String>) -> Self {
    self.name = Some(Source::Fixed(name.into()));

    self
  }

  /// Derives the accessible name the caption supplies from the signals `name` reads.
  #[inline(always)]
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

impl<P: Platform> From<String> for Label<P> {
  #[inline(always)]
  fn from(text: String) -> Self {
    Self {
      view: Text::new(text.clone()).into_view(),
      name: Some(Source::Fixed(text)),
    }
  }
}

impl<P: Platform> From<&str> for Label<P> {
  #[inline(always)]
  fn from(text: &str) -> Self {
    Self::from(text.to_owned())
  }
}

/// The drawn body a control holds: nothing yet, or one view.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a control's body",
  label = "a control holds `NoChild` or the one view `set_child` gave it"
)]
pub trait ControlChild<P: Platform>: 'static {
  /// Builds the body, if there is one.
  fn build_body(self) -> Option<NodeId>;
}

impl<P: Platform> ControlChild<P> for NoChild {
  #[inline(always)]
  fn build_body(self) -> Option<NodeId> {
    None
  }
}

impl<P: Platform> ControlChild<P> for AnyView<P> {
  #[inline(always)]
  fn build_body(self) -> Option<NodeId> {
    Some(self.build())
  }
}

/// The handlers a control's user installs, each observing what the behavior did.
#[derive(Default)]
struct Observers {
  click: Vec<Rc<PlainHandler>>,
  key_down: Vec<Rc<KeyObserver>>,
  key_up: Vec<Rc<KeyObserver>>,
  pointer_down: Vec<Rc<PointerObserver>>,
  pointer_up: Vec<Rc<PointerObserver>>,
  pointer_move: Vec<Rc<PointerObserver>>,
  pointer_enter: Vec<Rc<PlainHandler>>,
  pointer_leave: Vec<Rc<PlainHandler>>,
  focus: Vec<Rc<PlainHandler>>,
  blur: Vec<Rc<PlainHandler>>,
  scroll: Vec<Rc<ScrollHandler>>,
}

/// A control: focus, events, decoration, a label, parts and accessibility once, over
/// the [`Behavior`] that says what the interaction is.
///
/// The behavior runs first on every event it acts on and its verdict is the whole
/// verdict; every `handle_` here observes. The control is controlled: it reports, and
/// the signal its behavior is bound to is the truth. Its parts and its child resolve
/// the control-scoped conditions (`Checked`, `Disabled`, …) and their focus against it.
/// Holds [`NoChild`] until [`set_child`](Self::set_child) or
/// [`set_child_builder`](Self::set_child_builder) supplies the drawn body; a control
/// with a label or parts alone builds without one. Its parts, its body and its label
/// sit in one row half an em apart, centered on the cross axis, the label where the
/// behavior places it ([`Behavior::label_placement`]): leading or trailing in the row,
/// or beside the box, before it among its siblings.
#[must_use]
pub struct Control<P: Platform, B: Behavior<P>, C = NoChild> {
  behavior: B,
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
  parts: Vec<(Part, AnyView<P>)>,
  observers: Observers,
  child: C,
}

impl<P: Platform, B: Behavior<P>, C> std::fmt::Debug for Control<P, B, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Control").finish_non_exhaustive()
  }
}

impl<P: Platform, B: Behavior<P>> Control<P, B> {
  /// A control driven by `behavior`, undecorated and without a body.
  #[inline(always)]
  pub fn new(behavior: B) -> Self {
    let mut inner = LayoutBox::new(Layout::Flex {
      direction: FlexDirection::Row,
    });

    inner.style_mut().item_alignment = Some(ItemAlignment::Center.into());
    inner.style_mut().spacing = Some(Expr::em(0.5).into());

    Self {
      behavior,
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
      parts: Vec::new(),
      observers: Observers::default(),
      child: NoChild,
    }
  }

  /// Sets the drawn body.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<P>) -> Control<P, B, AnyView<P>> {
    self.with_child(child.into_view())
  }

  /// Sets the drawn body, built by `child` under the control.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<P>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Control<P, B, AnyView<P>> {
    self.with_child(AnyView::new(move || child().into_view().build()))
  }

  fn with_child(self, child: AnyView<P>) -> Control<P, B, AnyView<P>> {
    Control {
      behavior: self.behavior,
      inner: self.inner,
      label: self.label,
      disabled: self.disabled,
      required: self.required,
      read_only: self.read_only,
      invalid: self.invalid,
      described_by: self.described_by,
      labelled_by: self.labelled_by,
      accessible_name: self.accessible_name,
      focus_order: self.focus_order,
      parts: self.parts,
      observers: self.observers,
      child,
    }
  }
}

impl<P: Platform, B: Behavior<P> + Default> Default for Control<P, B> {
  #[inline(always)]
  fn default() -> Self {
    Self::new(B::default())
  }
}

impl<P: Platform, B: Behavior<P>, C> Control<P, B, C> {
  /// The declared style, for a composite control to write the decoration it forwards
  /// into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }

  decoration! { style_mut }

  /// Sets the axis the control's parts, body and label run along; horizontal unless
  /// told otherwise.
  #[inline(always)]
  pub fn set_axis(mut self, axis: Axis) -> Self {
    self.inner = self.inner.set_layout(Layout::Flex {
      direction: match axis {
        Axis::Horizontal => FlexDirection::Row,
        Axis::Vertical => FlexDirection::Column,
      },
    });

    self
  }

  properties! { style_mut;
    set_spacing / derive_spacing (spacing) => spacing: Spacing<P::Scalar>,
      "the spacing between the control's parts, body and label; half an em unless told otherwise";
    set_item_alignment / derive_item_alignment (alignment) => item_alignment: ItemAlignment,
      "where the control's parts, body and label sit on the cross axis; centered unless told otherwise";
  }

  /// Binds `anchor` to the control once built, so an overlay is placed against it.
  #[inline(always)]
  pub fn bind_anchor(mut self, anchor: &Anchor) -> Self {
    self.inner = self.inner.bind_anchor(anchor);

    self
  }

  /// Binds `focus` to the control once built, so focus moves to it programmatically.
  #[inline(always)]
  pub fn bind_focus(mut self, focus: &FocusHandle<P>) -> Self {
    self.inner = self.inner.bind_focus(focus);

    self
  }

  /// The behavior driving the control.
  #[inline(always)]
  pub const fn behavior(&self) -> &B {
    &self.behavior
  }

  /// Sets where the body sits when it fills neither axis.
  #[inline(always)]
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    self.inner = self.inner.set_alignment(alignment);

    self
  }

  /// Derives where the body sits from the signals `alignment` reads.
  #[inline(always)]
  pub fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    mut self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.inner = self.inner.derive_alignment(alignment);

    self
  }

  /// Names the control for container queries against its size.
  #[inline(always)]
  pub fn set_container_name(mut self, name: ContainerName) -> Self {
    self.inner = self.inner.set_container_name(name);

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.inner = self.inner.set_transition(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<BoxKeyframe<P>>) -> Self {
    self.inner = self.inner.set_animation(animation);

    self
  }

  /// Names the box for view transitions: it is paired with the box of the same name
  /// across a navigation, where the platform animates between them.
  #[inline(always)]
  pub fn set_transition_name(mut self, name: impl Into<String>) -> Self {
    self.inner = self.inner.set_transition_name(name);

    self
  }

  /// Sets the caption: a [`Part::Label`] child whose text is the accessible name.
  ///
  /// A string builds a [`Text`] and names the control; [`Label::new`] mounts any view
  /// and leaves the name to the platform.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<Label<P>>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets the name the control reports to assistive technology, over its caption's: for
  /// a control no caption names, such as a button drawn by an icon alone.
  #[inline(always)]
  pub fn set_accessible_name(mut self, name: impl Into<String>) -> Self {
    self.accessible_name = Some(Source::Fixed(name.into()));

    self
  }

  /// Derives the name the control reports to assistive technology from the signals
  /// `name` reads.
  #[inline(always)]
  pub fn derive_accessible_name(mut self, name: impl Fn() -> String + 'static) -> Self {
    self.accessible_name = Some(Source::derive(name));

    self
  }

  /// Sets whether the control is present but not operable: a disabled control takes
  /// no focus, ignores every event it would act on and reports
  /// [`AccessProp::Disabled`].
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.disabled = Some(Source::Fixed(disabled));

    self
  }

  /// Derives whether the control is disabled from the signals `disabled` reads.
  #[inline(always)]
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Some(Source::derive(disabled));

    self
  }

  /// Sets whether a value must be supplied before the form the control is in submits.
  #[inline(always)]
  pub fn set_required(mut self, required: bool) -> Self {
    self.required = Some(Source::Fixed(required));

    self
  }

  /// Derives whether the control is required from the signals `required` reads.
  #[inline(always)]
  pub fn derive_required(mut self, required: impl Fn() -> bool + 'static) -> Self {
    self.required = Some(Source::derive(required));

    self
  }

  /// Sets whether the control's value is shown but not editable.
  #[inline(always)]
  pub fn set_read_only(mut self, read_only: bool) -> Self {
    self.read_only = Some(Source::Fixed(read_only));

    self
  }

  /// Derives whether the control is read-only from the signals `read_only` reads.
  #[inline(always)]
  pub fn derive_read_only(mut self, read_only: impl Fn() -> bool + 'static) -> Self {
    self.read_only = Some(Source::derive(read_only));

    self
  }

  /// Sets whether the control holds an invalid value; the `Invalid` state inside it
  /// follows.
  #[inline(always)]
  pub fn set_invalid(mut self, invalid: bool) -> Self {
    self.invalid = Some(Source::Fixed(invalid));

    self
  }

  /// Derives whether the control is invalid from the signals `invalid` reads.
  #[inline(always)]
  pub fn derive_invalid(mut self, invalid: impl Fn() -> bool + 'static) -> Self {
    self.invalid = Some(Source::derive(invalid));

    self
  }

  /// Sets the node whose text describes the control, for assistive technology.
  #[inline(always)]
  pub const fn set_described_by(mut self, node: NodeId) -> Self {
    self.described_by = Some(node);

    self
  }

  /// Names the control by the text of `node`, a caption placed anywhere, after the
  /// caption [`set_label`](Self::set_label) builds, if any.
  #[inline(always)]
  pub const fn set_labelled_by(mut self, node: NodeId) -> Self {
    self.labelled_by = Some(node);

    self
  }

  /// Sets the control's place in focus traversal, over the behavior's default.
  #[inline(always)]
  pub fn set_focus_order(mut self, focus_order: FocusOrder) -> Self {
    self.focus_order = Some(Source::Fixed(focus_order));

    self
  }

  /// Derives the control's place in focus traversal from the signals `focus_order`
  /// reads, over the behavior's default.
  #[inline(always)]
  pub fn derive_focus_order(mut self, focus_order: impl Fn() -> FocusOrder + 'static) -> Self {
    self.focus_order = Some(Source::derive(focus_order));

    self
  }

  /// Adds `view` as the part `part` of the control, a styling target its conditions
  /// resolve against the control.
  #[inline(always)]
  pub fn set_part(mut self, part: Part, view: impl IntoView<P>) -> Self {
    self.parts.push((part, view.into_view()));

    self
  }

  /// Runs `action` after the behavior has acted on a click.
  #[inline(always)]
  pub fn handle_click(mut self, action: impl Fn() + 'static) -> Self {
    self.observers.click.push(Rc::new(action));

    self
  }

  /// Runs `action` after the behavior has acted on a key pressed while the control
  /// holds focus.
  #[inline(always)]
  pub fn handle_key_down(mut self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.observers.key_down.push(Rc::new(action));

    self
  }

  /// Runs `action` on a key released while the control holds focus.
  #[inline(always)]
  pub fn handle_key_up(mut self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.observers.key_up.push(Rc::new(action));

    self
  }

  /// Runs `action` after the behavior has acted on a pointer button pressed over the
  /// control.
  #[inline(always)]
  pub fn handle_pointer_down(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.observers.pointer_down.push(Rc::new(action));

    self
  }

  /// Runs `action` after the behavior has acted on a pointer button released over the
  /// control.
  #[inline(always)]
  pub fn handle_pointer_up(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.observers.pointer_up.push(Rc::new(action));

    self
  }

  /// Runs `action` after the behavior has acted on the pointer moving over the control.
  #[inline(always)]
  pub fn handle_pointer_move(mut self, action: impl Fn(&PointerEvent) + 'static) -> Self {
    self.observers.pointer_move.push(Rc::new(action));

    self
  }

  /// Runs `action` when the pointer enters the control.
  #[inline(always)]
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.observers.pointer_enter.push(Rc::new(action));

    self
  }

  /// Runs `action` when the pointer leaves the control.
  #[inline(always)]
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.observers.pointer_leave.push(Rc::new(action));

    self
  }

  /// Runs `action` when focus enters the control or a node inside it from outside.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.observers.focus.push(Rc::new(action));

    self
  }

  /// Runs `action` when focus leaves the control and every node inside it.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.observers.blur.push(Rc::new(action));

    self
  }

  /// Runs `action` with the control's new offsets when it scrolls.
  #[inline(always)]
  pub fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.observers.scroll.push(Rc::new(action));

    self
  }
}

impl<P: Platform, B: Behavior<P>, C> AnyKind for Control<P, B, C> {}

impl<P: Platform, B: Behavior<P>, C: ControlChild<P>> View<P> for Control<P, B, C> {
  fn build(self) -> NodeId {
    let Self {
      behavior,
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
      parts,
      observers,
      child,
    } = self;
    let behavior = Rc::new(behavior);
    let owner = Owner::current().child();

    owner.enter(move || {
      let kind = behavior.kind();
      let placeholder = behavior.placeholder();
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

      let node = inner.realize(NodeKind::Control, |box_props| {
        P::create_control(&ControlProps {
          kind,
          box_props,
          label: name.as_ref().map(|name| untrack(|| name())),
          placeholder,
          lines,
          label_beside: placement == LabelPlacement::Beside,
        })
      });
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
        platform: PhantomData,
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
      };

      bind_reports(&handle, &behavior, &flags, focus_order);
      behavior.attach(&handle);
      register(node, &behavior, &handle, &disabled, observers);

      let mut label = label;

      if placement == LabelPlacement::Leading
        && let Some((part, _)) = label.take()
      {
        mount_part::<P>(node, Part::Label, part);
      }

      for (part, view) in parts {
        mount_part::<P>(node, part, view.build());
      }

      if let Some(child) = child.build_body() {
        Scene::<P>::with(|scene| scene.attach(node, child, None));
      }

      if let Some((part, _)) = label {
        mount_part::<P>(node, Part::Label, part);
      }

      node
    })
  }
}

/// A boolean setting as a closure: `false` when unset.
fn read_bool(source: Option<Source<bool>>) -> Rc<dyn Fn() -> bool> {
  source.map_or_else(|| Rc::new(|| false), Source::into_read)
}

/// Attaches `child` under `node` as its `part`.
fn mount_part<P: Platform>(node: NodeId, part: Part, child: NodeId) {
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
}

/// Binds the render effects writing the control's state, accessibility and focus
/// order, each re-applied when what it reads changes or the handle is refreshed.
fn bind_reports<P: Platform, B: Behavior<P>>(
  handle: &Rc<ControlHandle<P>>,
  behavior: &Rc<B>,
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
        access.push(AccessProp::Label(name()));
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

      with_sink::<P, _>(node, |sink| P::update_focus_order(sink, order));
    });
  }
}

/// What every acting handler shares: the behavior, its handle and the disabled read.
struct Actor<P: Platform, B: Behavior<P>> {
  behavior: Rc<B>,
  handle: Rc<ControlHandle<P>>,
  disabled: Rc<dyn Fn() -> bool>,
}

impl<P: Platform, B: Behavior<P>> Clone for Actor<P, B> {
  fn clone(&self) -> Self {
    Self {
      behavior: Rc::clone(&self.behavior),
      handle: Rc::clone(&self.handle),
      disabled: Rc::clone(&self.disabled),
    }
  }
}

impl<P: Platform, B: Behavior<P>> Actor<P, B> {
  /// Runs `report`, the behavior hearing a change the platform made itself, unless the
  /// control is disabled, then reconciles the platform with what the behavior holds.
  fn report(&self, report: impl FnOnce(&B, &ControlHandle<P>)) {
    if !(self.disabled)() {
      report(&self.behavior, &self.handle);
    }

    self.handle.refresh();
    self.behavior.reconcile(&self.handle);
  }

  /// One handler for an event carrying `E`: a disabled control neither acts nor lets the
  /// observers see it and gives `when_disabled` as its verdict, else the
  /// behavior acts through `act` when `acts`, the observers see the event after, and
  /// the behavior's verdict stands.
  ///
  /// `None` when neither the behavior nor an observer wants the event.
  fn handler<E: ?Sized + 'static>(
    &self,
    when_disabled: Prevent,
    acts: bool,
    act: impl Fn(&B, &E, &ControlHandle<P>) -> Prevent + 'static,
    observers: Vec<Rc<Observer<E>>>,
  ) -> Option<Rc<Acting<E>>> {
    if !acts && observers.is_empty() {
      return None;
    }

    let actor = self.clone();

    Some(Rc::new(move |event: &E| {
      if (actor.disabled)() {
        return when_disabled;
      }

      let verdict = if acts {
        act(&actor.behavior, event, &actor.handle)
      }
      else {
        Prevent::None
      };

      for observer in &observers {
        observer(event);
      }

      verdict
    }))
  }
}

/// Registers one handler per event the behavior or an observer wants: the behavior
/// acts first and its verdict stands, the observers see the event after, a key an input
/// method is composing never reaches the behavior, and a disabled control swallows
/// every pointer event and click and lets every key by.
#[expect(
  clippy::too_many_lines,
  reason = "one registration per event kind; length is the kind count, not complexity"
)]
fn register<P: Platform, B: Behavior<P>>(
  node: NodeId,
  behavior: &Rc<B>,
  handle: &Rc<ControlHandle<P>>,
  disabled: &Rc<dyn Fn() -> bool>,
  observers: Observers,
) {
  let listens = behavior.listens();
  let acts = |kind: EventKind| listens.contains(&kind);
  let actor = Actor {
    behavior: Rc::clone(behavior),
    handle: Rc::clone(handle),
    disabled: Rc::clone(disabled),
  };
  let Observers {
    click,
    key_down,
    key_up,
    pointer_down,
    pointer_up,
    pointer_move,
    pointer_enter,
    pointer_leave,
    focus,
    blur,
    scroll,
  } = observers;
  let acting_click = actor.handler(
    Prevent::All,
    acts(EventKind::Click),
    |behavior, (): &(), handle| behavior.on_click(handle),
    Vec::new(),
  );
  let click = actor.handler(
    Prevent::All,
    false,
    |_, (): &(), _| Prevent::None,
    click
      .into_iter()
      .map(|observer| -> Rc<Observer<()>> { Rc::new(move |()| observer()) })
      .collect(),
  );
  let key_down = actor.handler(
    Prevent::None,
    acts(EventKind::KeyDown),
    |behavior: &B, event: &KeyEvent, handle| {
      if event.composing {
        Prevent::None
      }
      else {
        behavior.on_key(event, handle)
      }
    },
    key_down,
  );
  let key_up = actor.handler(
    Prevent::None,
    acts(EventKind::KeyUp),
    |behavior: &B, event: &KeyEvent, handle| {
      if event.composing {
        Prevent::None
      }
      else {
        behavior.on_key_up(event, handle)
      }
    },
    key_up,
  );
  let pointer = |phase: PointerPhase, kind: EventKind, observers: Vec<Rc<PointerObserver>>| {
    actor.handler(
      Prevent::All,
      acts(kind),
      move |behavior: &B, event: &PointerEvent, handle| behavior.on_pointer(phase, event, handle),
      observers,
    )
  };
  let pointer_down = pointer(PointerPhase::Down, EventKind::PointerDown, pointer_down);
  let pointer_up = pointer(PointerPhase::Up, EventKind::PointerUp, pointer_up);
  let pointer_move = pointer(PointerPhase::Move, EventKind::PointerMove, pointer_move);
  let change = acts(EventKind::Change).then(|| {
    let actor = actor.clone();

    Rc::new(move |value: &str| {
      actor.report(|behavior, handle| behavior.on_change(value, handle));
    })
  });
  let selection_change = acts(EventKind::SelectionChange).then(|| {
    let actor = actor.clone();

    Rc::new(move |selection: TextSelection| {
      actor.report(|behavior, handle| behavior.on_selection_change(selection, handle));
    })
  });
  let toggle = acts(EventKind::Toggle).then(|| {
    let actor = actor.clone();

    Rc::new(move |checked: bool| {
      actor.report(|behavior, handle| behavior.on_toggle(checked, handle));
    })
  });
  let acting_blur = acts(EventKind::Blur).then(|| {
    let behavior = Rc::clone(behavior);
    let handle = Rc::clone(handle);

    Rc::new(move || behavior.on_blur(&handle))
  });

  Scene::<P>::with(|scene| {
    if let Some(handler) = acting_click {
      scene.handle_click_acting(node, Rc::new(move || handler(&())));
    }

    if let Some(handler) = click {
      scene.handle_click(node, Rc::new(move || handler(&())));
    }

    if let Some(handler) = key_down {
      scene.handle_key_down(node, handler);
    }

    if let Some(handler) = key_up {
      scene.handle_key_up(node, handler);
    }

    if let Some(handler) = pointer_down {
      scene.handle_pointer_down(node, as_pointer_handler(handler));
    }

    if let Some(handler) = pointer_up {
      scene.handle_pointer_up(node, as_pointer_handler(handler));
    }

    if let Some(handler) = pointer_move {
      scene.handle_pointer_move(node, as_pointer_handler(handler));
    }

    if let Some(handler) = change {
      scene.handle_change(node, handler);
    }

    if let Some(handler) = selection_change {
      scene.handle_selection_change(node, handler);
    }

    if let Some(handler) = toggle {
      scene.handle_toggle(node, handler);
    }

    for observer in pointer_enter {
      scene.handle_pointer_enter(node, observer);
    }

    for observer in pointer_leave {
      scene.handle_pointer_leave(node, observer);
    }

    for observer in focus {
      scene.handle_focus(node, observer);
    }

    if let Some(handler) = acting_blur {
      scene.handle_blur(node, handler);
    }

    for observer in blur {
      scene.handle_blur(node, observer);
    }

    for observer in scroll {
      scene.handle_scroll(node, observer);
    }
  });
}

/// A pointer handler as dispatch stores it.
#[inline(always)]
fn as_pointer_handler(handler: Rc<Acting<PointerEvent>>) -> Rc<PointerHandler> {
  handler
}

/// A button: [`Control`] over [`Activate`].
pub type RawButton<P, C = NoChild> = Control<P, Activate, C>;
/// A checkbox: [`Control`] over [`Toggle`].
pub type RawCheckbox<P, C = NoChild> = Control<P, Toggle, C>;
/// A slider: [`Control`] over [`Range`].
pub type RawSlider<P, C = NoChild> = Control<P, Range, C>;
/// A radio group: [`Control`] over [`Roving`].
pub type RawRadioGroup<P, C = NoChild> = Control<P, Roving, C>;
/// One radio choice: [`Control`] over [`RadioMember`].
pub type RawRadio<P, C = NoChild> = Control<P, RadioMember, C>;
/// A single-line text field: [`Control`] over [`TextEntry`].
pub type RawTextField<P, C = NoChild> = Control<P, TextEntry, C>;
/// A multi-line text editor: [`Control`] over [`TextEntry`] with
/// [`set_lines`](TextEntry::set_lines) above one.
pub type RawTextEditor<P, C = NoChild> = Control<P, TextEntry, C>;
/// A listbox: [`Control`] over [`Listbox`].
pub type RawListbox<P, C = NoChild> = Control<P, Listbox, C>;
/// One option within a listbox: [`Control`] over [`ListItem`].
pub type RawOption<P, C = NoChild> = Control<P, ListItem, C>;
/// A menu: [`Control`] over [`Menu`].
pub type RawMenu<P, C = NoChild> = Control<P, Menu, C>;
/// One item within a menu: [`Control`] over [`MenuOption`].
pub type RawMenuItem<P, C = NoChild> = Control<P, MenuOption, C>;
