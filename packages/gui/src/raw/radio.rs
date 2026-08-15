//! A mutually-exclusive choice, drawn by the views it holds.

use std::rc::Rc;

use skene_core::{
  AccessProp,
  AnyView,
  FocusOrder,
  IntoView,
  KeyEvent,
  NodeId,
  Prevent,
  Ring,
  Role,
  Roster,
  Source,
  View,
};
use skene_reactive::{Owner, on_cleanup, provide_context, use_context};

use crate::{
  Container,
  FocusHandle,
  HasEvents,
  control::{Behavior, EventCore},
};

/// What a [`RawRadioGroup`] shares with the members inside it.
///
/// Reached through context rather than a typed child kind, so members may sit at any
/// depth—wrapped for layout, interleaved with decoration—and still belong to the group.
#[derive(Clone)]
struct GroupContext {
  chosen: Rc<dyn Fn() -> Option<String>>,
  choose: Rc<dyn Fn(String)>,
  /// Members in join order, which the arrow keys walk; one leaves as it is disposed,
  /// so a reconciling set never strands a destroyed member in traversal.
  members: Roster<FocusHandle>,
}

impl GroupContext {
  /// Moves `step` places from the member holding `value` and chooses what it lands on,
  /// focusing it as the browser's own group does.
  fn step_from(&self, value: &str, step: isize) {
    let Some(index) = self.members.position_of(Some(value))
    else {
      return;
    };
    let Some(next) = Ring::new(self.members.count()).step_from(index, step)
    else {
      return;
    };
    let Some((value, handle)) = self.members.at(next)
    else {
      return;
    };

    (self.choose)(value);
    handle.focus();
  }

  /// Whether `value` is the group's one tab stop: the chosen member, or the first when
  /// nothing is chosen yet.
  fn is_tab_stop(&self, value: &str) -> bool {
    match (self.chosen)() {
      Some(chosen) => chosen == value,
      None => self.members.leads_with(value),
    }
  }
}

/// A group of mutually-exclusive choices, holding the views that draw them.
///
/// The group owns the selection and the keyboard: exactly one member is checked, the
/// arrow keys move between members and choose as they go, and the whole group is one
/// tab stop—all of it what the browser's own radio group does. Members find the group
/// through context, so they may sit at any depth inside it and be wrapped or
/// interleaved with anything.
///
/// The selection flows both ways: [`derive_chosen`](Self::derive_chosen) (or
/// [`set_chosen`](Self::set_chosen)) drives which member is checked, and
/// [`handle_choose`](Self::handle_choose) receives each change—
/// [`bind_chosen`](Self::bind_chosen) wires both to one signal.
#[must_use]
pub struct RawRadioGroup<C = skene_core::NoChild> {
  child: C,
  chosen: Option<Source<Option<String>>>,
  choose_handler: Option<Rc<dyn Fn(String)>>,
  label: Option<String>,
}

impl RawRadioGroup {
  /// Creates a group with nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      child: skene_core::NoChild,
      chosen: None,
      choose_handler: None,
      label: None,
    }
  }
}

impl Default for RawRadioGroup {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<C> RawRadioGroup<C> {
  /// Sets the child holding the members, which the group requires before it can build.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView) -> RawRadioGroup<AnyView> {
    RawRadioGroup {
      child: child.into_view(),
      chosen: self.chosen,
      choose_handler: self.choose_handler,
      label: self.label,
    }
  }

  /// Sets the chosen member's value.
  #[inline(always)]
  pub fn set_chosen(mut self, chosen: impl Into<String>) -> Self {
    self.chosen = Some(Source::Static(Some(chosen.into())));

    self
  }

  /// Derives the chosen member's value, re-reported whenever it changes.
  #[inline(always)]
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<String> + 'static) -> Self {
    self.chosen = Some(Source::Derived(Rc::new(chosen)));

    self
  }

  /// Sets the choose handler, run with the new value on every change.
  #[inline(always)]
  pub fn handle_choose(mut self, action: impl Fn(String) + 'static) -> Self {
    self.choose_handler = Some(Rc::new(action));

    self
  }

  /// Binds the selection to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_chosen(move || Some(signal.get()))
      .handle_choose(move |value| signal.set(value))
  }

  /// Sets the group's accessible name.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl View for RawRadioGroup<AnyView> {
  fn build(self) -> NodeId {
    let chosen = self.chosen;
    let handler = self.choose_handler;
    let context = GroupContext {
      chosen: Rc::new(move || chosen.as_ref().and_then(Source::get)),
      choose: Rc::new(move |value| {
        if let Some(action) = &handler {
          action(value);
        }
      }),
      members: Roster::new(),
    };
    let label = self.label;

    // A child owner keeps the context to this group's subtree, so a sibling group's
    // members never see it.
    Owner::current().child().enter(move || {
      provide_context(context);

      let mut group = Container::new().set_role(Role::RadioGroup);

      if let Some(label) = label {
        group = group.set_accessibility([AccessProp::Label(label)]);
      }

      group.set_child(self.child).build()
    })
  }
}

/// One choice within a [`RawRadioGroup`], drawn by the view it holds.
///
/// Carries its value and nothing else: the group it finds through context owns the
/// selection, the keyboard, and the single tab stop. Outside a group it still draws,
/// reports its role, and is never checked—so a stray member is inert rather than
/// broken.
///
/// The group's keyboard and the choice on click are overridable per member, since a
/// handler installed with [`handle_click`](Self::handle_click) or
/// [`handle_key_down`](Self::handle_key_down) runs ahead of them and may take the
/// event.
#[must_use]
pub struct RawRadio<C = skene_core::NoChild> {
  child: C,
  value: String,
  events: EventCore,
  label: Option<String>,
}

impl RawRadio {
  /// Creates a choice carrying `value`, the string the group reports when it is chosen.
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

impl<C> HasEvents for RawRadio<C> {
  #[inline(always)]
  fn event_core(&mut self) -> &mut EventCore {
    &mut self.events
  }
}

impl<C> RawRadio<C> {
  /// Sets the child that draws the choice, which it requires before it can build.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView) -> RawRadio<AnyView> {
    RawRadio {
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

impl View for RawRadio<AnyView> {
  fn build(self) -> NodeId {
    let group: Option<GroupContext> = use_context();
    let value = self.value;
    let label = self.label;
    let handle = FocusHandle::new();
    let state = {
      let group = group.clone();
      let value = value.clone();
      let label = label.clone();

      move || {
        let chosen = group
          .as_ref()
          .is_some_and(|group| (group.chosen)().as_deref() == Some(value.as_str()));
        let mut state = vec![AccessProp::Checked(chosen.into())];

        if let Some(label) = &label {
          state.push(AccessProp::Label(label.clone()));
        }

        state
      }
    };
    let order = {
      let group = group.clone();
      let value = value.clone();

      move || match &group {
        // One tab stop for the whole group: every other member is reachable by arrow
        // key, exactly as a native group behaves.
        Some(group) if !group.is_tab_stop(&value) => FocusOrder::Skip,
        _ => FocusOrder::Auto,
      }
    };
    let behavior = Behavior::new()
      .click({
        let group = group.clone();
        let value = value.clone();

        move || {
          if let Some(group) = &group {
            (group.choose)(value.clone());
          }

          Prevent::None
        }
      })
      .key_down({
        let group = group.clone();
        let value = value.clone();

        move |event: &KeyEvent| {
          let Some(group) = &group
          else {
            return Prevent::None;
          };

          match event.key.as_str() {
            "ArrowRight" | "ArrowDown" => group.step_from(&value, 1),
            "ArrowLeft" | "ArrowUp" => group.step_from(&value, -1),
            " " => (group.choose)(value.clone()),
            _ => return Prevent::None,
          }

          Prevent::Platform
        }
      });
    let container = Container::new()
      .set_role(Role::Radio)
      .derive_focus_order(order)
      .derive_accessibility(state)
      .bind_focus(&handle);
    let node = self
      .events
      .attach(container, behavior)
      .set_child(self.child)
      .build();

    if let Some(group) = &group {
      group.members.join(value.clone(), handle);

      let group = group.clone();

      on_cleanup(move || group.members.leave(&value));
    }

    node
  }
}
