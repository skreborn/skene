//! A choice among fixed options, drawn by the views it holds.

use std::rc::Rc;

use skene_core::{
  AccessProp,
  AnyView,
  FocusOrder,
  IntoChildren,
  IntoView,
  KeyEvent,
  NodeId,
  Placement,
  PlainChild,
  Prevent,
  Role,
  Source,
  View,
};
use skene_reactive::{Get, Owner, Set, Signal, on_cleanup, provide_context, use_context};

use crate::{
  Anchor,
  Children,
  Column,
  Container,
  FocusHandle,
  HasChildren,
  HasEvents,
  Overlay,
  Roster,
  Stack,
  control::{Behavior, EventCore},
};

/// What a [`RawPicker`] shares with the options inside it.
///
/// Reached through context rather than a typed child kind, so options may sit at any
/// depth—grouped under a heading, separated, wrapped for layout—and still belong to the
/// picker.
#[derive(Clone)]
struct PickerContext {
  chosen: Rc<dyn Fn() -> Option<String>>,
  choose: Rc<dyn Fn(String)>,
  /// The option the keyboard sits on while the list is open, committed to only on
  /// request—so dismissing leaves the old value in place.
  active: Signal<Option<String>>,
  open: Signal<bool>,
  /// Options in document order, which the arrow keys walk—however the set is filtered,
  /// and never reaching one that has been destroyed.
  members: Roster<FocusHandle>,
  face: FocusHandle,
}

impl PickerContext {
  /// The place `step` from `at`, clamped at both ends the way a native select's list
  /// is—unlike a radio group, which wraps.
  fn clamped(count: usize, at: usize, step: isize) -> Option<usize> {
    count
      .checked_sub(1)
      .map(|last| at.saturating_add_signed(step).min(last))
  }

  /// Where `value` sits among `members`, or the first place when it is not among them.
  fn position_in(members: &[(String, FocusHandle)], value: Option<&str>) -> usize {
    value
      .and_then(|value| members.iter().position(|(held, _)| held == value))
      .unwrap_or(0)
  }

  /// Highlights one option and moves focus onto it, so a reader announces the option
  /// the keyboard has reached.
  fn highlight(&self, option: Option<&(String, FocusHandle)>) {
    let Some((value, handle)) = option
    else {
      return;
    };

    self.active.set(Some(value.clone()));
    handle.focus();
  }

  /// Moves the highlight `step` places without committing.
  fn step_highlight(&self, step: isize) {
    let members = self.members.ordered();
    let at = Self::position_in(&members, self.active.get().as_deref());

    if let Some(next) = Self::clamped(members.len(), at, step) {
      self.highlight(members.get(next));
    }
  }

  /// Opens the list with the highlight on the chosen option, or the first when nothing
  /// is chosen.
  fn open_list(&self) {
    let members = self.members.ordered();
    let at = Self::position_in(&members, (self.chosen)().as_deref());

    self.open.set(true);
    self.highlight(members.get(at));
  }

  /// Closes the list and returns focus to the face, so the control keeps the focus it
  /// was given.
  fn close(&self) {
    self.open.set(false);
    self.face.focus();
  }

  /// Commits the highlighted option and closes.
  fn commit(&self) {
    if let Some(value) = self.active.get() {
      (self.choose)(value);
    }

    self.close();
  }

  /// Commits `value` outright, as a press on one option does.
  fn commit_value(&self, value: String) {
    self.active.set(Some(value.clone()));
    (self.choose)(value);
    self.close();
  }

  /// Chooses the option `step` places from the chosen one without opening, the way a
  /// closed native select answers the arrow keys.
  fn cycle(&self, step: isize) {
    let members = self.members.ordered();
    let at = Self::position_in(&members, (self.chosen)().as_deref());
    let Some(next) = Self::clamped(members.len(), at, step)
    else {
      return;
    };
    let Some((value, _)) = members.get(next)
    else {
      return;
    };

    (self.choose)(value.clone());
  }

  /// Whether `value` is the list's one tab stop: the highlighted option, while open.
  fn is_tab_stop(&self, value: &str) -> bool {
    self.open.get() && self.active.get().as_deref() == Some(value)
  }
}

/// A choice among fixed options, drawn by the views it holds: the control supplies the
/// behavior, the children supply every pixel.
///
/// The face draws the control closed; the options draw the list it opens. Nothing is
/// drawn for you, so both come from [`set_face`](Self::set_face) and
/// [`set_options`](Self::set_options)—properties first, the large one last.
///
/// Closed, `Enter`, `Space`, `Alt`+`ArrowDown`, and a click open the list, and the
/// arrows, `Home`, and `End` change the value outright without opening. Open, the arrows
/// move the highlight and focus with it, `Enter`, `Space`, and `Tab` commit it, and
/// `Escape` or a press outside dismisses with the value unchanged—all of it what the
/// browser's own select does. Type-ahead is the one thing missing, since resetting its
/// buffer needs a timer and timers are userland.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives which option is chosen, and
/// [`handle_change`](Self::handle_change) receives each choice—
/// [`bind_value`](Self::bind_value) wires both to one signal.
///
/// The handlers this control carries cover the face. An option carries its own, since
/// the keyboard reaches it directly while the list is open.
#[must_use]
pub struct RawPicker<F = skene_core::NoChild> {
  face: F,
  options: Children,
  value: Option<Source<String>>,
  change_handler: Option<Rc<dyn Fn(String)>>,
  events: EventCore,
  focus_order: FocusOrder,
  label: Option<String>,
}

impl RawPicker {
  /// Creates a picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      face: skene_core::NoChild,
      options: Children::default(),
      value: None,
      change_handler: None,
      events: EventCore::default(),
      focus_order: FocusOrder::Auto,
      label: None,
    }
  }
}

impl Default for RawPicker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<F> HasEvents for RawPicker<F> {
  #[inline(always)]
  fn event_core(&mut self) -> &mut EventCore {
    &mut self.events
  }
}

impl<F> RawPicker<F> {
  /// Sets the view drawing the control closed, which it requires before it can build.
  #[inline(always)]
  pub fn set_face(self, face: impl IntoView) -> RawPicker<AnyView> {
    RawPicker {
      face: face.into_view(),
      options: self.options,
      value: self.value,
      change_handler: self.change_handler,
      events: self.events,
      focus_order: self.focus_order,
      label: self.label,
    }
  }

  /// Sets the views drawing the list, each [`RawOption`] among them an option.
  ///
  /// Anything else here draws and nothing more, so a heading, a separator, or a wrapper
  /// around an option is free to sit between them.
  #[inline(always)]
  pub fn set_options(mut self, options: impl IntoChildren<PlainChild>) -> Self {
    self.options = Children::new(options);

    self
  }

  /// Sets a static chosen value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Static(value.into()));

    self
  }

  /// Sets a derived chosen value, re-reported whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Source::Derived(Rc::new(value)));

    self
  }

  /// Sets the change handler, run with the newly chosen value.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(String) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }

  /// Binds the value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(self, signal: Signal<String>) -> Self {
    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }

  /// Sets the control's place in sequential focus traversal.
  #[inline(always)]
  pub fn set_focus_order(mut self, focus_order: FocusOrder) -> Self {
    self.focus_order = focus_order;

    self
  }

  /// Sets the accessible name, for when the face draws no readable label.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl View for RawPicker<AnyView> {
  fn build(self) -> NodeId {
    let value = self.value;
    let handler = self.change_handler;
    let open = Signal::new(false);
    let context = PickerContext {
      chosen: Rc::new(move || value.as_ref().map(Source::get)),
      choose: Rc::new(move |chosen| {
        if let Some(action) = &handler {
          action(chosen);
        }
      }),
      active: Signal::new(None),
      open,
      members: Roster::new(),
      face: FocusHandle::new(),
    };
    let label = self.label;
    let anchor = Anchor::new();
    let root = context.members.clone();

    // A child owner keeps the context to this picker's subtree, so a sibling picker's
    // options never see it.
    Owner::current().child().enter(move || {
      provide_context(context.clone());

      let face = {
        let state = {
          let context = context.clone();
          let label = label.clone();

          move || {
            let mut state = vec![AccessProp::Expanded(context.open.get())];

            if let Some(label) = &label {
              state.push(AccessProp::Label(label.clone()));
            }

            state
          }
        };
        let behavior = Behavior::new()
          .click({
            let context = context.clone();

            move || {
              context.open_list();

              Prevent::Bubbling
            }
          })
          .key_down({
            let context = context.clone();

            move |event: &KeyEvent| face_key(&context, event)
          });
        let container = Container::new()
          .set_role(Role::Combobox)
          .set_focus_order(self.focus_order)
          .derive_accessibility(state)
          .bind_anchor(&anchor)
          .bind_focus(&context.face);

        self.events.attach(container, behavior).set_child(self.face)
      };
      let list = Overlay::popup()
        .set_anchor(&anchor)
        .set_placement(Placement::Bottom)
        .derive_open(move || open.get())
        .handle_dismiss({
          let context = context.clone();

          move || context.close()
        })
        .set_children((Container::new()
          .set_role(Role::Listbox)
          .set_child(Column::new().set_children_to(self.options)),));

      let node = Stack::new().set_children((face, list)).build();

      root.root_at(node);

      node
    })
  }
}

/// Where a key takes a closed picker, which answers the arrows without opening.
fn face_key(context: &PickerContext, event: &KeyEvent) -> Prevent {
  match event.key.as_str() {
    "Enter" | " " => context.open_list(),
    "ArrowDown" if event.modifiers.alt => context.open_list(),
    "ArrowDown" => context.cycle(1),
    "ArrowUp" => context.cycle(-1),
    "Home" => context.cycle(isize::MIN),
    "End" => context.cycle(isize::MAX),
    _ => return Prevent::None,
  }

  Prevent::All
}

/// Where a key takes an open picker, from the option the keyboard sits on.
fn option_key(context: &PickerContext, event: &KeyEvent) -> Prevent {
  match event.key.as_str() {
    "ArrowUp" if event.modifiers.alt => context.commit(),
    "ArrowDown" => context.step_highlight(1),
    "ArrowUp" => context.step_highlight(-1),
    "Home" => context.step_highlight(isize::MIN),
    "End" => context.step_highlight(isize::MAX),
    "Enter" | " " | "Tab" => context.commit(),
    "Escape" => context.close(),
    _ => return Prevent::None,
  }

  Prevent::All
}

/// One option within a [`RawPicker`], drawn by the view it holds.
///
/// Carries its value and nothing else: the picker it finds through context owns the
/// selection, the highlight, and the list's single tab stop. Outside a picker it still
/// draws, reports its role, and is never selected—so a stray option is inert rather
/// than broken.
///
/// The keyboard reaches an option directly while the list is open, so the handlers it
/// carries cover the open list's arrows, commit, and dismissal.
#[must_use]
pub struct RawOption<C = skene_core::NoChild> {
  child: C,
  value: String,
  events: EventCore,
  label: Option<String>,
}

impl RawOption {
  /// Creates an option carrying `value`, the string the picker reports when it is
  /// chosen.
  #[inline(always)]
  pub fn new(value: impl Into<String>) -> Self {
    Self {
      child: skene_core::NoChild,
      value: value.into(),
      events: EventCore::default(),
      label: None,
    }
  }
}

impl<C> HasEvents for RawOption<C> {
  #[inline(always)]
  fn event_core(&mut self) -> &mut EventCore {
    &mut self.events
  }
}

impl<C> RawOption<C> {
  /// Sets the child that draws the option, which it requires before it can build.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView) -> RawOption<AnyView> {
    RawOption {
      child: child.into_view(),
      value: self.value,
      events: self.events,
      label: self.label,
    }
  }

  /// Sets the accessible name, for when the child draws no readable label.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl View for RawOption<AnyView> {
  fn build(self) -> NodeId {
    let picker: Option<PickerContext> = use_context();
    let value = self.value;
    let label = self.label;
    let handle = FocusHandle::new();
    let state = {
      let picker = picker.clone();
      let value = value.clone();
      let label = label.clone();

      move || {
        let chosen = picker
          .as_ref()
          .is_some_and(|picker| (picker.chosen)().as_deref() == Some(value.as_str()));
        let mut state = vec![AccessProp::Selected(chosen)];

        if let Some(label) = &label {
          state.push(AccessProp::Label(label.clone()));
        }

        state
      }
    };
    let order = {
      let picker = picker.clone();
      let value = value.clone();

      move || match &picker {
        // One tab stop for the whole list, moving with the highlight, exactly as a
        // radio group's moves with its selection.
        Some(picker) if !picker.is_tab_stop(&value) => FocusOrder::Skip,
        _ => FocusOrder::Auto,
      }
    };
    let behavior = Behavior::new()
      .click({
        let picker = picker.clone();
        let value = value.clone();

        move || {
          if let Some(picker) = &picker {
            picker.commit_value(value.clone());
          }

          Prevent::Bubbling
        }
      })
      .pointer_enter({
        let picker = picker.clone();
        let value = value.clone();

        move || {
          if let Some(picker) = &picker {
            picker.active.set(Some(value.clone()));
          }
        }
      })
      .key_down({
        let picker = picker.clone();

        move |event: &KeyEvent| {
          picker
            .as_ref()
            .map_or(Prevent::None, |picker| option_key(picker, event))
        }
      });
    let container = Container::new()
      .set_role(Role::ListboxOption)
      .derive_focus_order(order)
      .derive_accessibility(state)
      .bind_focus(&handle);
    let node = self
      .events
      .attach(container, behavior)
      .set_child(self.child)
      .build();

    if let Some(picker) = &picker {
      picker.members.join(node, value, handle);

      let picker = picker.clone();

      on_cleanup(move || picker.members.leave(node));
    }

    node
  }
}
