//! A ready-made dropdown choice in the Simple design language.

use std::rc::Rc;

use skene_core::{Condition, HasChildren, ItemAlignment, Source, View};

use crate::{
  Column,
  Container,
  Enhanced,
  Inset,
  RawOption,
  RawPicker,
  Row,
  Text,
  ThemedBox,
  ThemedText,
  WebPicker,
  simple,
};

/// The indicator after the chosen label.
const INDICATOR: &str = "▾";
/// The gap between the label and the indicator.
const INDICATOR_GAP: f32 = 8.;

/// A ready-made select in the Simple design language.
///
/// Drawn by the framework once the client owns the page: the field, the popup, and each
/// row are ordinary views, so the list wears the Simple surface rather than the
/// platform's dropdown chrome—which no engine lets a page style freely. Without script
/// the browser's own select stands in, which works on its own.
///
/// The chosen value flows both ways. For a select drawn some other way, compose
/// [`RawPicker`] directly.
#[must_use]
pub struct SimplePicker {
  options: Vec<(String, String)>,
  value: Option<Source<String>>,
  change_handler: Option<Rc<dyn Fn(String)>>,
}

impl SimplePicker {
  /// Creates a select with no options.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      options: Vec::new(),
      value: None,
      change_handler: None,
    }
  }

  /// Sets the options, in order, each a `(value, label)` pair.
  #[inline(always)]
  pub fn set_options<V: Into<String>, L: Into<String>>(
    mut self,
    options: impl IntoIterator<Item = (V, L)>,
  ) -> Self {
    self.options = options
      .into_iter()
      .map(|(value, label)| (value.into(), label.into()))
      .collect();

    self
  }

  /// Sets a static chosen value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Static(value.into()));

    self
  }

  /// Sets a derived chosen value, re-applied whenever it changes.
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

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }
}

impl Default for SimplePicker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for SimplePicker {
  fn build(self) -> skene_core::NodeId {
    let Self {
      options,
      value,
      change_handler,
    } = self;
    let first = options.first().map(|(value, _)| value.clone());
    let reads = move || {
      value
        .as_ref()
        .map(Source::get)
        .or_else(|| first.clone())
        .unwrap_or_default()
    };

    Enhanced::new({
      let options = options.clone();
      let reads = reads.clone();
      let change_handler = change_handler.clone();

      move || native(options.clone(), reads.clone(), change_handler.clone())
    })
    .set_child(move || drawn(options.clone(), reads.clone(), change_handler.clone()))
    .build()
  }
}

/// The scriptless form: the browser's own select, styled as close to the drawn one as
/// an engine allows.
fn native(
  options: Vec<(String, String)>,
  value: impl Fn() -> String + 'static,
  change_handler: Option<Rc<dyn Fn(String)>>,
) -> WebPicker {
  use crate::ThemedControl;

  let mut control = simple::field_chrome(
    WebPicker::new()
      .set_options(options)
      .derive_value(value)
      .theme_color(|palette| palette.card),
  );

  if let Some(action) = change_handler {
    control = control.handle_change(move |value| action(value.to_owned()));
  }

  control
}

/// The drawn form: the field, and a popup wearing the Simple surface.
fn drawn(
  options: Vec<(String, String)>,
  value: impl Fn() -> String + Clone + 'static,
  change_handler: Option<Rc<dyn Fn(String)>>,
) -> impl View {
  let labels = options.clone();
  let shown = value.clone();
  let face = Container::new()
    .set_padding(Inset::symmetric(12., 8.))
    .set_border_radius(simple::RADIUS)
    .theme_color(|palette| palette.card)
    .theme_border(1., |palette| palette.border)
    .theme_border_when(Condition::Focused, 1., |palette| palette.ring)
    .set_child(
      Row::new()
        .set_spacing(INDICATOR_GAP)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          Text::derive(move || {
            let chosen = shown();

            labels
              .iter()
              .find(|(value, _)| *value == chosen)
              .map_or(chosen, |(_, label)| label.clone())
          }),
          Text::new(INDICATOR).theme_color(|palette| palette.muted_foreground),
        )),
    );
  // The overlay carries no surface of its own, so the list paints one.
  let list = Container::new()
    .set_border_radius(simple::RADIUS)
    .theme_color(|palette| palette.card)
    .theme_border(1., |palette| palette.border)
    .set_padding(Inset::all(4.))
    .set_child(
      Column::new().set_children(
        options
          .into_iter()
          .map(|(value, label)| row(value, label))
          .collect::<Vec<_>>(),
      ),
    );
  let mut control = RawPicker::new().derive_value(value);

  if let Some(action) = change_handler {
    control = control.handle_change(move |value| action(value));
  }

  control.set_face(face).set_options((list,))
}

/// One row of the drawn list: the highlight is the row's own focus, since the keyboard
/// moves focus with it.
fn row(value: String, label: String) -> impl View {
  RawOption::new(value).set_child(
    Container::new()
      .set_padding(Inset::symmetric(8., 6.))
      .set_border_radius(4.)
      .theme_color_when(Condition::Focused, |palette| palette.primary.surface)
      .set_child(Text::new(label)),
  )
}
