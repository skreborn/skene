//! A single-line text input.

use std::rc::Rc;

use skene_core::{NodeId, Source, View};

use crate::{control::ControlBase, state::NodeKind, with_state};

/// A single-line text input.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the shown text, and
/// [`handle_change`](Self::handle_change) receives each edit in real time—
/// [`bind_value`](Self::bind_value) wires both to one signal. Styling is the box style,
/// set wholesale with [`set_style`](Self::set_style) and conditionally with
/// [`set_when`](Self::set_when).
#[must_use]
pub struct Input {
  base: ControlBase<crate::props::BoxStyle>,
  value: Option<Source<String>>,
  placeholder: Option<String>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
}

impl Input {
  /// Creates an empty input.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      value: None,
      placeholder: None,
      change_handler: None,
    }
  }

  /// Sets a static value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Static(value.into()));

    self
  }

  /// Sets a derived value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Source::Derived(Rc::new(value)));

    self
  }

  /// Binds the value to `signal` in both directions: the input shows the signal and each
  /// edit writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value.to_owned()))
  }

  /// Sets the hint shown while the value is empty.
  #[inline(always)]
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.placeholder = Some(placeholder.into());

    self
  }

  /// Sets the change handler, run with the new value on every edit, in real time.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }
}

impl Default for Input {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

crate::control::control_surface!(Input, crate::props::BoxStyle);

impl View for Input {
  fn build(mut self) -> NodeId {
    let style = self.base.style;
    let variants = std::mem::take(&mut self.base.variants);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Input {
          style,
          variants,
          value: String::new(),
          placeholder: self.placeholder,
        },
        taffy::style::Style::default(),
      )
    });

    if let Some(value) = self.value {
      value.bind(move |value| set_input_value(node, &value));
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

/// Sets the live value of an input node.
fn set_input_value(node: NodeId, value: &str) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Input { value: current, .. } = &mut record.kind {
        value.clone_into(current);
      }
    });

    state.dirty.set(true);
  });
}
