//! A ready-made radio group and its members, in the Simple design language.

use std::rc::Rc;

use skene_core::{
  Alignment,
  AnyView,
  Border,
  Condition,
  HasChildren,
  IntoView,
  ItemAlignment,
  NodeId,
  Source,
  View,
};
use skene_reactive::{Owner, provide_context, use_context};

use crate::{Container, Enhanced, RawRadio, RawRadioGroup, Row, SimplePalette, Text, WebRadio};

/// The ring's diameter.
const RING_SIZE: f32 = 16.;
/// The dot's diameter inside it.
const DOT_SIZE: f32 = 8.;
/// The gap between the ring and its caption.
const CAPTION_GAP: f32 = 8.;

/// What a [`SimpleRadioGroup`] shares with its members.
///
/// Carries what only the scriptless form needs—the shared name that makes the browser's
/// own radios exclusive. The drawn form takes its selection from the enclosing
/// [`RawRadioGroup`]'s own context instead.
#[derive(Clone)]
struct GroupName {
  name: String,
  chosen: Rc<dyn Fn() -> Option<String>>,
  choose: Rc<dyn Fn(String)>,
}

/// A ready-made group of radio buttons in the Simple design language.
///
/// The group owns the selection and the keyboard; members find it by sitting inside it,
/// at any depth, so one may be wrapped or interleaved with anything. It carries the
/// name the scriptless form needs to make the browser's own radios exclusive, and the
/// context the drawn form selects through.
#[must_use]
pub struct SimpleRadioGroup<C = skene_core::NoChild> {
  name: String,
  chosen: Option<Source<String>>,
  change_handler: Option<Rc<dyn Fn(String)>>,
  label: Option<String>,
  child: C,
}

impl SimpleRadioGroup {
  /// Creates a group named `name`, with nothing chosen.
  ///
  /// The name is what the browser's own radios share to be exclusive, so it must be
  /// unique among the groups on a page.
  #[inline(always)]
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      chosen: None,
      change_handler: None,
      label: None,
      child: skene_core::NoChild,
    }
  }
}

impl<C> SimpleRadioGroup<C> {
  /// Sets the child holding the members, which the group requires before it can build.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView) -> SimpleRadioGroup<AnyView> {
    SimpleRadioGroup {
      name: self.name,
      chosen: self.chosen,
      change_handler: self.change_handler,
      label: self.label,
      child: child.into_view(),
    }
  }

  /// Sets the chosen member's value.
  #[inline(always)]
  pub fn set_chosen(mut self, chosen: impl Into<String>) -> Self {
    self.chosen = Some(Source::Static(chosen.into()));

    self
  }

  /// Derives the chosen member's value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_chosen(mut self, chosen: impl Fn() -> String + 'static) -> Self {
    self.chosen = Some(Source::Derived(Rc::new(chosen)));

    self
  }

  /// Sets the change handler, run with the newly chosen value.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(String) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }

  /// Binds the selection to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_chosen(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }

  /// Sets the group's accessible name.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl View for SimpleRadioGroup<AnyView> {
  fn build(self) -> NodeId {
    let Self {
      name,
      chosen,
      change_handler,
      label,
      child,
    } = self;
    let reads = {
      let chosen = chosen.clone();

      move || chosen.as_ref().map(Source::get)
    };
    let writes: Rc<dyn Fn(String)> = Rc::new(move |value| {
      if let Some(action) = &change_handler {
        action(value);
      }
    });
    let shared = GroupName {
      name,
      chosen: Rc::new(reads.clone()),
      choose: Rc::clone(&writes),
    };
    let mut group = RawRadioGroup::new().derive_chosen(reads);

    if let Some(label) = label {
      group = group.set_label(label);
    }

    // A child owner keeps the name to this group's members, as the raw group does with
    // its own selection context.
    Owner::current().child().enter(move || {
      provide_context(shared);

      group
        .handle_choose(move |value| writes(value))
        .set_child(child)
        .build()
    })
  }
}

/// A ready-made radio button in the Simple design language.
///
/// Drawn by the framework once the client owns the page: the ring and its dot are
/// ordinary views. Without script the browser's own radio stands in, made exclusive by
/// the enclosing [`SimpleRadioGroup`]'s name.
///
/// Outside a group it still draws and is never chosen, so a stray member is inert
/// rather than broken. For a radio drawn some other way, compose [`RawRadio`] directly.
#[must_use]
pub struct SimpleRadio {
  value: String,
  label: Option<String>,
}

impl SimpleRadio {
  /// Creates the member standing for `value`.
  #[inline(always)]
  pub fn new(value: impl Into<String>) -> Self {
    Self {
      value: value.into(),
      label: None,
    }
  }

  /// Sets the caption beside the ring; activating it selects too.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl View for SimpleRadio {
  fn build(self) -> NodeId {
    let Self { value, label } = self;
    let group: Option<GroupName> = use_context();

    Enhanced::new({
      let group = group.clone();
      let value = value.clone();
      let label = label.clone();

      move || native(group.clone(), value.clone(), label.clone())
    })
    .set_child(move || drawn(group.clone(), value.clone(), label.clone()))
    .build()
  }
}

/// The scriptless form: the browser's own radio, exclusive through the shared name.
fn native(group: Option<GroupName>, value: String, label: Option<String>) -> WebRadio {
  let name = group
    .as_ref()
    .map_or_else(String::new, |group| group.name.clone());
  let chosen = group.clone();
  let held = value.clone();
  let mut control = WebRadio::new(name, value.clone()).derive_checked(move || {
    chosen.as_ref().and_then(|group| (group.chosen)()) == Some(held.clone())
  });

  if let Some(label) = label {
    control = control.set_label(label);
  }

  if let Some(group) = group {
    control = control.handle_toggle(move |checked| {
      if checked {
        (group.choose)(value.clone());
      }
    });
  }

  control
}

/// The drawn form: a ring that carries a dot when chosen, beside the caption.
fn drawn(group: Option<GroupName>, value: String, label: Option<String>) -> impl View {
  let chosen = {
    let value = value.clone();

    move || {
      group
        .as_ref()
        .is_some_and(|group| (group.chosen)().as_deref() == Some(value.as_str()))
    }
  };
  let dotted = chosen.clone();
  let ring = Container::new()
    .set_width(RING_SIZE)
    .set_height(RING_SIZE)
    .set_border_radius(RING_SIZE / 2.)
    .set_border(Border::new(1., SimplePalette::border()))
    .set_border_when(Condition::Focused, Border::new(1., SimplePalette::ring()))
    .set_color(SimplePalette::card())
    .set_alignment(Alignment::CENTER)
    .set_child(
      Container::new()
        .set_width(DOT_SIZE)
        .set_height(DOT_SIZE)
        .set_border_radius(DOT_SIZE / 2.)
        .derive_color(move || {
          if dotted() {
            SimplePalette::primary()
          }
          else {
            SimplePalette::card()
          }
        }),
    );
  let mut control = RawRadio::new(value);

  if let Some(label) = label.clone() {
    control = control.set_label(label);
  }

  match label {
    Some(label) => control.set_child(
      Row::new()
        .set_spacing(CAPTION_GAP)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((ring, Text::new(label))),
    ),
    None => control.set_child(ring),
  }
}
