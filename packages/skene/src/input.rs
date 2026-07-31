// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::{Animation, BoxStyle, Condition, KeyEvent, Source, Transition};

/// A single-line text input.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the shown text, and
/// [`handle_input`](Self::handle_input) receives each edit —
/// [`bind_value`](Self::bind_value) wires both to one signal. Styling is the box style,
/// set wholesale with [`set_style`](Self::set_style) and conditionally with
/// [`set_when`](Self::set_when).
///
/// ```ignore
/// let name = RwSignal::new(String::new());
///
/// Input::new()
///   .set_placeholder("Name")
///   .bind_value(name)
/// ```
#[must_use]
pub struct Input {
  pub(crate) value: Option<Source<String>>,
  pub(crate) placeholder: Option<String>,
  pub(crate) style: BoxStyle,
  pub(crate) variants: Vec<(Vec<Condition>, BoxStyle)>,
  pub(crate) transitions: Vec<Transition>,
  pub(crate) animations: Vec<Animation<BoxStyle>>,
  #[expect(clippy::struct_field_names, clippy::type_complexity)]
  pub(crate) input_handler: Option<Rc<dyn Fn(&str)>>,
  #[expect(clippy::type_complexity)]
  pub(crate) key_down_handler: Option<Rc<dyn Fn(&KeyEvent)>>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
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
      transitions: Vec::new(),
      animations: Vec::new(),
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

  /// Adds a property transition, interpolating that property whenever its value changes.
  #[inline(always)]
  pub fn set_transition(mut self, transition: Transition) -> Self {
    self.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<BoxStyle>) -> Self {
    self.animations.push(animation);

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

#[cfg(target_arch = "wasm32")]
impl Input {
  /// Binds the value to `signal` in both directions: the input shows the signal and each
  /// edit writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::RwSignal<String>) -> Self {
    use skene_reactive::{SignalGet, SignalUpdate};

    self
      .derive_value(move || signal.get())
      .handle_input(move |value| signal.set(value.to_owned()))
  }
}

#[cfg(target_arch = "wasm32")]
impl skene_core::View for Input {
  fn build(self) -> skene_core::NodeId {
    use skene_core::InputProps;

    use crate::driver;

    let node = driver::create_input(&InputProps {
      placeholder: self.placeholder,
      style: self.style,
      variants: self.variants,
      transitions: self.transitions,
      animations: self.animations,
    });

    if let Some(value) = self.value {
      value.bind(move |value| driver::set_input_value(node, &value));
    }

    if let Some(action) = self.input_handler {
      driver::handle_input(node, action);
    }

    if let Some(action) = self.key_down_handler {
      driver::handle_key_down(node, action);
    }

    if let Some(action) = self.focus_handler {
      driver::handle_focus(node, action);
    }

    if let Some(action) = self.blur_handler {
      driver::handle_blur(node, action);
    }

    node
  }
}
