//! A single-line text input.

use std::rc::Rc;

use skene_core::{KeyEvent, NodeId, Source, View};

use crate::{
  props::{BoxStyle, Condition},
  state::NodeKind,
  with_state,
};

/// A single-line text input.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the shown text, and
/// [`handle_input`](Self::handle_input) receives each edit—
/// [`bind_value`](Self::bind_value) wires both to one signal. Styling is the box style,
/// set wholesale with [`set_style`](Self::set_style) and conditionally with
/// [`set_when`](Self::set_when).
#[must_use]
pub struct Input {
  value: Option<Source<String>>,
  placeholder: Option<String>,
  style: BoxStyle,
  variants: Vec<(Vec<Condition>, BoxStyle)>,
  #[expect(clippy::struct_field_names, clippy::type_complexity)]
  input_handler: Option<Rc<dyn Fn(&str)>>,
  #[expect(clippy::type_complexity)]
  key_down_handler: Option<Rc<dyn Fn(&KeyEvent)>>,
  focus_handler: Option<Rc<dyn Fn()>>,
  blur_handler: Option<Rc<dyn Fn()>>,
}

impl Input {
  /// Creates an empty input.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      value: None,
      placeholder: None,
      style: BoxStyle::new(),
      variants: Vec::new(),
      input_handler: None,
      key_down_handler: None,
      focus_handler: None,
      blur_handler: None,
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
  pub fn bind_value(self, signal: skene_reactive::RwSignal<String>) -> Self {
    use skene_reactive::{SignalGet, SignalUpdate};

    self
      .derive_value(move || signal.get())
      .handle_input(move |value| signal.set(value.to_owned()))
  }

  /// Sets the hint shown while the value is empty.
  #[inline(always)]
  pub fn set_placeholder(mut self, placeholder: impl Into<String>) -> Self {
    self.placeholder = Some(placeholder.into());

    self
  }

  /// Sets the static style.
  #[inline(always)]
  pub fn set_style(mut self, style: BoxStyle) -> Self {
    self.style = style;

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.variants.push((vec![condition], style));

    self
  }

  /// Adds a conditional style variant, applied while all of `conditions` hold.
  #[inline(always)]
  pub fn set_when_all(
    mut self,
    conditions: impl IntoIterator<Item = Condition>,
    style: BoxStyle,
  ) -> Self {
    self
      .variants
      .push((conditions.into_iter().collect(), style));

    self
  }

  /// Sets the edit handler, run with the new value on every change.
  #[inline(always)]
  pub fn handle_input(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.input_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler.
  #[inline(always)]
  pub fn handle_key_down(mut self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.key_down_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.blur_handler = Some(Rc::new(action));

    self
  }
}

impl Default for Input {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for Input {
  fn build(self) -> NodeId {
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Input {
          style: self.style,
          variants: self.variants,
          value: String::new(),
          placeholder: self.placeholder,
        },
        taffy::style::Style::default(),
      )
    });

    if let Some(value) = self.value {
      value.bind(move |value| set_input_value(node, &value));
    }

    with_state(|state| {
      if let Some(action) = self.input_handler {
        state
          .input
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.key_down_handler {
        state
          .key_down
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.focus_handler {
        state
          .focus
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.blur_handler {
        state
          .blur
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }
    });

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
