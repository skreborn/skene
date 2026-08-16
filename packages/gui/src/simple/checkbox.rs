//! A ready-made checkbox in the Simple design language.

use std::rc::Rc;

use skene_core::{Alignment, Border, Condition, HasChildren, ItemAlignment, Source, View};

use crate::{
  Container,
  Enhanced,
  HasDecoration,
  RawCheckbox,
  Row,
  SimplePalette,
  Text,
  WebCheckbox,
};

/// The box's edge length.
const BOX_SIZE: f32 = 16.;
/// The gap between the box and its caption.
const CAPTION_GAP: f32 = 8.;

/// A ready-made checkbox in the Simple design language.
///
/// Drawn by the framework, so it looks the same on every GUI platform: the box, its
/// border, and the mark are ordinary views. A page rendered without script carries the
/// browser's own checkbox instead—which works on its own—and the drawn one replaces it
/// once the client owns the page; see [`Enhanced`].
///
/// The checked state flows both ways. For a checkbox drawn some other way, compose
/// [`RawCheckbox`] directly.
#[must_use]
pub struct SimpleCheckbox {
  checked: Option<Source<bool>>,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
  label: Option<String>,
}

impl SimpleCheckbox {
  /// Creates an unchecked checkbox.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      checked: None,
      toggle_handler: None,
      label: None,
    }
  }

  /// Sets the caption beside the box; activating it toggles too.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets a static checked state.
  #[inline(always)]
  pub fn set_checked(mut self, checked: bool) -> Self {
    self.checked = Some(Source::Static(checked));

    self
  }

  /// Sets a derived checked state, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.checked = Some(Source::Derived(Rc::new(checked)));

    self
  }

  /// Sets the toggle handler, run with the new state on every change.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.toggle_handler = Some(Rc::new(action));

    self
  }

  /// Binds the state to `signal` in both directions: the checkbox shows the signal and
  /// each toggle writes it back.
  #[inline(always)]
  pub fn bind_checked(self, signal: skene_reactive::Signal<bool>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_checked(move || signal.get())
      .handle_toggle(move |checked| signal.set(checked))
  }
}

impl Default for SimpleCheckbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for SimpleCheckbox {
  fn build(self) -> skene_core::NodeId {
    let Self {
      checked,
      toggle_handler,
      label,
    } = self;
    let reads = {
      let checked = checked.clone();

      move || checked.as_ref().is_some_and(Source::get)
    };

    Enhanced::new({
      let reads = reads.clone();
      let toggle_handler = toggle_handler.clone();
      let label = label.clone();

      move || native(reads.clone(), toggle_handler.clone(), label.clone())
    })
    .set_child(move || drawn(reads.clone(), toggle_handler.clone(), label.clone()))
    .build()
  }
}

/// The scriptless form: the browser's own checkbox, which needs no script to work.
fn native(
  checked: impl Fn() -> bool + 'static,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
  label: Option<String>,
) -> WebCheckbox {
  // The same footprint as the drawn box, so the swap at hydration shifts nothing.
  let mut control = WebCheckbox::new()
    .derive_checked(checked)
    .set_width(BOX_SIZE)
    .set_height(BOX_SIZE)
    .set_border_radius(4.);

  if let Some(label) = label {
    control = control.set_label(label);
  }

  if let Some(action) = toggle_handler {
    control = control.handle_toggle(move |checked| action(checked));
  }

  control
}

/// The drawn form: an ordinary box and mark, beside the caption.
fn drawn(
  checked: impl Fn() -> bool + Clone + 'static,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
  label: Option<String>,
) -> impl View {
  let filled = checked.clone();
  let marked = checked.clone();
  let box_view = Container::new()
    .set_width(BOX_SIZE)
    .set_height(BOX_SIZE)
    .set_border_radius(4.)
    .set_border(Border::new(1., SimplePalette::border()))
    // The focus ring is the field's, so a drawn control reads as one of the family.
    .set_border_when(
      Condition::Focused,
      Border::new(1., SimplePalette::ring()),
    )
    .derive_color(move || {
      if filled() {
        SimplePalette::primary()
      }
      else {
        SimplePalette::card()
      }
    })
    .set_alignment(Alignment::CENTER)
    .set_child(
      Text::derive(move || if marked() { "✓" } else { "" }.to_owned())
        .set_color(SimplePalette::primary_foreground()),
    );
  let mut control = RawCheckbox::new().derive_checked(checked);

  if let Some(action) = toggle_handler {
    control = control.handle_toggle(move |checked| action(checked));
  }

  if let Some(label) = label.clone() {
    control = control.set_label(label);
  }

  match label {
    Some(label) => control.set_child(
      Row::new()
        .set_spacing(CAPTION_GAP)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((box_view, Text::new(label))),
    ),
    None => control.set_child(box_view),
  }
}
