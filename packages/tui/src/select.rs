//! A fixed-option selector, cycling in place, in cells.

use std::rc::Rc;

use skene_core::{NodeId, Source, View};

use crate::{control::ControlBase, state::NodeKind, theme::Glyphs, with_state};

/// A selector over fixed options, rendered as the current option's label.
///
/// `Enter`, `Space`, or a mouse click cycles to the next option; arrow keys cycle both
/// ways. The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) picks the shown option, and
/// [`handle_change`](Self::handle_change) receives each newly selected value—
/// [`bind_value`](Self::bind_value) wires both to one signal.
#[must_use]
pub struct Select {
  base: ControlBase<crate::props::ControlStyle>,
  options: Vec<(String, String)>,
  indicator: Option<char>,
  value: Option<Source<String>>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
}

impl Select {
  /// Creates a selector with no options.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      options: Vec::new(),
      indicator: None,
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

impl Default for Select {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

crate::control::control_surface!(Select, crate::props::ControlStyle);

impl View for Select {
  fn build(mut self) -> NodeId {
    let style = self.base.style;
    let variants = std::mem::take(&mut self.base.variants);
    let initial = self
      .options
      .first()
      .map(|(value, _)| value.clone())
      .unwrap_or_default();
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Select {
          style,
          variants,
          options: self.options,
          indicator: self
            .indicator
            .unwrap_or_else(|| Glyphs::current().select_indicator),
          value: initial,
        },
        taffy::style::Style::default(),
      )
    });

    if let Some(value) = self.value {
      value.bind(move |value| {
        with_state(|state| {
          state.with_node(node, |record| {
            if let NodeKind::Select { value: current, .. } = &mut record.kind {
              current.clone_from(&value);
            }
          });

          state.dirty.set(true);
        });
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

    self.base.register(node);

    node
  }
}
