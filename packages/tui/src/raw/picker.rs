//! A fixed-option selector with an options popup, in cells.

use std::rc::Rc;

use skene_core::{Bubbling, HasChildren, NodeId, Prevent, Source, View};

use crate::{
  Overlay,
  control::{Behavior, ControlBase},
  overlay::Anchor,
  props::BoxStyle,
  state::NodeKind,
  theme::Glyphs,
  with_state,
};

/// A selector over fixed options, rendered closed as the current option's label.
///
/// `Enter`, `Space`, or a mouse click opens the options popup beneath it. Open, the
/// arrows and the pointer move the highlight without committing; `Enter`, `Space`,
/// `Tab`, or a row click chooses the highlighted option, and `Escape` or a press
/// outside dismisses with the value unchanged. Closed, arrow keys cycle the value
/// directly.
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) picks the shown option, and
/// [`handle_change`](Self::handle_change) receives each newly selected value—
/// [`bind_value`](Self::bind_value) wires both to one signal.
///
/// The unstyled base a component library builds on; [`SimplePicker`](crate::SimplePicker) is
/// the ready-made counterpart.
#[must_use]
pub struct RawPicker {
  base: ControlBase<crate::props::ControlStyle>,
  options: Vec<(String, String)>,
  indicator: Option<char>,
  popup_style: Option<BoxStyle>,
  highlight: Option<(skene_core::Color, skene_core::Color)>,
  value: Option<Source<String>>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
}

impl RawPicker {
  /// Creates a selector with no options.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      options: Vec::new(),
      indicator: None,
      popup_style: None,
      highlight: None,
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

  /// Sets the highlighted row's bar and ink, replacing the default blue selection
  /// bar for the detected background.
  #[inline(always)]
  pub const fn set_highlight(
    mut self,
    background: skene_core::Color,
    color: skene_core::Color,
  ) -> Self {
    self.highlight = Some((background, color));

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

impl Default for RawPicker {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for RawPicker {
  fn build(mut self) -> NodeId {
    use skene_reactive::{Get, Set, Signal};

    let style = self.base.style.style;
    let variants = std::mem::take(&mut self.base.style.variants);
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

    // The popup highlights `active`, opening on the current option—only `Enter`,
    // `Space`, or a row click commits it, so `Escape` leaves the old value in place.
    let open = Signal::new(false);
    let active = Signal::new(selected.get_untracked());

    let behavior = Behavior::new()
      .click(popup_toggle(open, active, selected))
      .blur(move || open.set(false))
      .key_down(popup_keys(
        node,
        options.iter().map(|(value, _)| value.clone()).collect(),
        active,
        open,
      ));
    let anchor = Anchor::new();

    anchor.assign(node);

    let highlight = self.highlight;
    let popup = crate::theme::build_unselectable(|| {
      Overlay::popup()
        .set_anchor(&anchor)
        .set_placement(skene_core::Placement::Bottom)
        .bind_open(open)
        .set_style(self.popup_style.unwrap_or_else(default_popup_style))
        .set_child(
          crate::Column::new().set_children(
            options
              .into_iter()
              .map(|(value, label)| option_row(node, value, label, active, open, highlight))
              .collect::<Vec<_>>(),
          ),
        )
        .build()
    });

    with_state(|state| state.attach_child(node, popup, None));

    self.base.register(node, behavior);

    node
  }
}

/// The picker's own answer to activation: the popup toggles, opening with the highlight
/// on the current option.
fn popup_toggle(
  open: skene_reactive::Signal<bool>,
  active: skene_reactive::Signal<String>,
  selected: skene_reactive::Signal<String>,
) -> impl Fn() -> Prevent {
  use skene_reactive::{Get, Set, Update};

  move || {
    if !open.get() {
      active.set(selected.get());
    }

    open.update(|open| *open = !*open);

    // The popup is the picker's answer to a press: the terminal must not also cycle the
    // value, and no ancestor sees the click.
    Prevent::All
  }
}

/// The picker's own answer to a key: while open, arrows move the highlight without
/// committing, and `Enter`, `Space`, or `Tab` chooses it.
fn popup_keys(
  node: NodeId,
  values: Vec<String>,
  active: skene_reactive::Signal<String>,
  open: skene_reactive::Signal<bool>,
) -> impl Fn(&skene_core::KeyEvent) -> Prevent {
  use skene_reactive::{Get, Set};

  move |event: &skene_core::KeyEvent| {
    if !open.get() {
      return Prevent::None;
    }

    match event.key.as_str() {
      "ArrowDown" | "ArrowUp" => {
        let current = active.get();
        let at = values
          .iter()
          .position(|value| *value == current)
          .unwrap_or(0);
        let next = if event.key == "ArrowDown" {
          (at + 1).min(values.len().saturating_sub(1))
        }
        else {
          at.saturating_sub(1)
        };

        if let Some(value) = values.get(next) {
          active.set(value.clone());
        }

        Prevent::Platform
      }
      "Enter" | " " | "Tab" => {
        with_state(|state| state.select_picker(node, &active.get()));
        open.set(false);

        Prevent::Platform
      }
      _ => Prevent::None,
    }
  }
}

/// One selectable row of the options popup: the highlighted option carries the bar—
/// `highlight` when set, else the default for the detected background—following the
/// pointer, and a click commits.
fn option_row(
  node: NodeId,
  value: String,
  label: String,
  active: skene_reactive::Signal<String>,
  open: skene_reactive::Signal<bool>,
  highlight: Option<(skene_core::Color, skene_core::Color)>,
) -> crate::Container {
  use skene_core::Color;
  use skene_reactive::{Set, With};

  let shown = value.clone();
  let ink_shown = value.clone();
  let hovered = value.clone();

  crate::Container::new()
    .derive_color(move || {
      if active.with(|active| *active == shown) {
        highlight.map_or_else(
          || row_highlight(with_state(crate::state::State::dark)).0,
          |(background, _)| background,
        )
      }
      else {
        Color::srgb(0., 0., 0.).with_alpha(0.)
      }
    })
    .set_child(crate::Text::new(label).derive_color(move || {
      if active.with(|active| *active == ink_shown) {
        highlight.map_or_else(
          || row_highlight(with_state(crate::state::State::dark)).1,
          |(_, color)| color,
        )
      }
      else {
        Color::srgb(0., 0., 0.).with_alpha(0.)
      }
    }))
    .handle_pointer_enter(move || active.set(hovered.clone()))
    .handle_click(move || {
      with_state(|state| state.select_picker(node, &value));
      open.set(false);

      Bubbling::Stop
    })
}

/// The selected row's highlight bar—background and ink—for a dark or light terminal.
fn row_highlight(dark: bool) -> (skene_core::Color, skene_core::Color) {
  use skene_core::Color;

  if dark {
    (
      Color::srgb(52. / 255., 88. / 255., 134. / 255.),
      Color::srgb(1., 1., 1.),
    )
  }
  else {
    (
      Color::srgb(174. / 255., 208. / 255., 250. / 255.),
      Color::srgb(0., 0., 0.),
    )
  }
}

/// The popup's default box: a plain border in the scheme's ink, filled to cover the
/// content beneath.
fn default_popup_style() -> BoxStyle {
  let dark = with_state(crate::state::State::dark);
  let (backdrop, edge) = if dark {
    (
      skene_core::Color::srgb(0., 0., 0.),
      skene_core::Color::srgb(0.75, 0.75, 0.75),
    )
  }
  else {
    (
      skene_core::Color::srgb(1., 1., 1.),
      skene_core::Color::srgb(0.25, 0.25, 0.25),
    )
  };

  BoxStyle::new()
    .set_color(backdrop)
    .set_border(edge)
    .set_padding(crate::Inset::horizontal(1))
}

impl crate::HasFocus for RawPicker {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    &mut self.base.focus
  }
}

impl crate::HasMarks for RawPicker {
  #[inline(always)]
  fn mark_core(&mut self) -> &mut crate::MarkCore {
    &mut self.base.style
  }
}
