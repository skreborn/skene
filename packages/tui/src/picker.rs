//! A fixed-option selector with an options popup, in cells.

use std::rc::Rc;

use skene_core::{HasChildren, NodeId, Prevent, Source, View};

use crate::{
  Overlay,
  control::ControlBase,
  overlay::Anchor,
  props::BoxStyle,
  state::NodeKind,
  theme::Glyphs,
  with_state,
};

/// A selector over fixed options, rendered closed as the current option's label.
///
/// `Enter`, `Space`, or a mouse click opens the options popup beneath it (and closes
/// it again); clicking an option selects it, `Escape` or a press outside dismisses.
/// Arrow keys cycle the value directly, popup open or closed. The value flows both
/// ways: [`derive_value`](Self::derive_value) (or [`set_value`](Self::set_value))
/// picks the shown option, and [`handle_change`](Self::handle_change) receives each
/// newly selected value—[`bind_value`](Self::bind_value) wires both to one signal.
#[must_use]
pub struct Picker {
  base: ControlBase<crate::props::ControlStyle>,
  options: Vec<(String, String)>,
  indicator: Option<char>,
  popup_style: Option<BoxStyle>,
  value: Option<Source<String>>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
}

impl Picker {
  /// Creates a selector with no options.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      options: Vec::new(),
      indicator: None,
      popup_style: None,
      value: None,
      change_handler: None,
    }
  }

  /// Sets the options, each a `(value, label)` pair, in order.
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

  /// Sets the indicator after the current label, overriding any
  /// [`ControlTheme`](crate::ControlTheme).
  #[inline(always)]
  pub fn set_indicator(mut self, indicator: char) -> Self {
    self.indicator = Some(indicator);

    self
  }

  /// Sets the options popup's box style, replacing the default plain-bordered box
  /// filled to cover the content beneath.
  #[inline(always)]
  pub fn set_popup_style(mut self, style: BoxStyle) -> Self {
    self.popup_style = Some(style);

    self
  }

  /// Sets a static selected value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Static(value.into()));

    self
  }

  /// Sets a derived selected value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Source::Derived(Rc::new(value)));

    self
  }

  /// Sets the change handler, run with the newly selected value.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }

  /// Binds the selected value to `signal` in both directions: the selector shows the
  /// signal and each selection writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value.to_owned()))
  }
}

impl Default for Picker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

crate::control::control_surface!(Picker, crate::props::ControlStyle);

impl View for Picker {
  fn build(mut self) -> NodeId {
    use skene_reactive::{Set, Signal, Update};

    let style = self.base.style;
    let variants = std::mem::take(&mut self.base.variants);
    let initial = self
      .options
      .first()
      .map(|(value, _)| value.clone())
      .unwrap_or_default();
    let options = self.options.clone();
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Picker {
          style,
          variants,
          options: self.options,
          indicator: self
            .indicator
            .unwrap_or_else(|| Glyphs::current().picker_indicator),
          value: initial.clone(),
        },
        taffy::style::Style::default(),
      )
    });

    // The selection mirrored reactively, so the popup rows mark the current option
    // through every path: row clicks, arrow cycling, and external writes.
    let selected = Signal::new(initial);

    with_state(|state| {
      state
        .input
        .borrow_mut()
        .entry(node)
        .or_default()
        .push(Rc::new(move |value: &str| selected.set(value.to_owned())));
    });

    if let Some(value) = self.value {
      value.bind(move |value| {
        with_state(|state| {
          state.with_node(node, |record| {
            if let NodeKind::Picker { value: current, .. } = &mut record.kind {
              current.clone_from(&value);
            }
          });

          state.dirty.set(true);
        });

        selected.set(value);
      });
    }

    if let Some(action) = self.change_handler {
      with_state(|state| {
        state
          .input
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      });
    }

    // Activation toggles the options popup instead of the built-in cycle.
    let open = Signal::new(false);

    with_state(|state| {
      state
        .click
        .borrow_mut()
        .entry(node)
        .or_default()
        .push(Rc::new(move || {
          open.update(|open| *open = !*open);

          Prevent::Default
        }));
    });

    let anchor = Anchor::new();

    anchor.assign(node);

    let popup = crate::theme::build_unselectable(|| {
      Overlay::popup()
        .set_anchor(&anchor)
        .bind_open(open)
        .set_style(self.popup_style.unwrap_or_else(default_popup_style))
        .set_child(
          crate::Column::new().set_children(
            options
              .into_iter()
              .map(|(value, label)| option_row(node, value, label, selected, open))
              .collect::<Vec<_>>(),
          ),
        )
        .build()
    });

    with_state(|state| state.attach_child(node, popup, None));

    self.base.register(node);

    node
  }
}

/// One selectable row of the options popup: the current option carries the mark.
fn option_row(
  node: NodeId,
  value: String,
  label: String,
  selected: skene_reactive::Signal<String>,
  open: skene_reactive::Signal<bool>,
) -> crate::Container {
  use skene_reactive::{Set, With};

  let shown = value.clone();

  crate::Container::new()
    .set_child(crate::Text::derive(move || {
      let mark = selected.with(|selected| if *selected == shown { 'x' } else { ' ' });

      format!("{mark} {label}")
    }))
    .handle_click(move || {
      with_state(|state| state.select_picker(node, &value));
      open.set(false);

      Prevent::Default
    })
}

/// The popup's default box: a plain border, filled to cover the content beneath.
fn default_popup_style() -> BoxStyle {
  let dark = with_state(|state| state.dark.get());
  let backdrop = if dark {
    skene_core::Color::srgb(0., 0., 0.)
  }
  else {
    skene_core::Color::srgb(1., 1., 1.)
  };

  BoxStyle::new()
    .set_color(backdrop)
    .set_border(skene_core::Color::srgb(0., 0., 0.).with_alpha(0.))
    .set_padding(crate::Inset::horizontal(1))
}
