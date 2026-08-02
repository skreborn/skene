//! A numeric slider on a horizontal track, in cells.

use std::rc::Rc;

use skene_core::{NodeId, Source, View};

use crate::{control::ControlBase, state::NodeKind, theme::Glyphs, with_state};

/// A slider over `min..=max`, rendered as a track with a thumb (`──●──`).
///
/// Arrow keys move the thumb by one [step](Self::set_step); a mouse click jumps it to
/// the clicked cell. The value flows both ways: [`derive_value`](Self::derive_value)
/// (or [`set_value`](Self::set_value)) positions the thumb, and
/// [`handle_change`](Self::handle_change) receives each new value—
/// [`bind_value`](Self::bind_value) wires both to one signal.
#[must_use]
pub struct Slider {
  base: ControlBase,
  min: f32,
  max: f32,
  step: f32,
  glyphs: Option<(char, char)>,
  value: Option<Source<f32>>,
  change_handler: Option<Rc<dyn Fn(f32)>>,
}

impl Slider {
  /// Creates a slider over `min..=max`, stepping by 1.
  #[inline(always)]
  pub fn new(min: f32, max: f32) -> Self {
    Self {
      base: ControlBase::new(),
      min,
      max,
      step: 1.0,
      glyphs: None,
      value: None,
      change_handler: None,
    }
  }

  /// Sets the step a key press moves by, and the grid values snap to.
  #[inline(always)]
  pub fn set_step(mut self, step: f32) -> Self {
    self.step = step;

    self
  }

  /// Sets the track and thumb glyphs, overriding any
  /// [`ControlTheme`](crate::ControlTheme).
  #[inline(always)]
  pub fn set_track(mut self, track: char, thumb: char) -> Self {
    self.glyphs = Some((track, thumb));

    self
  }

  /// Sets a static value.
  #[inline(always)]
  pub fn set_value(mut self, value: f32) -> Self {
    self.value = Some(Source::Static(value));

    self
  }

  /// Sets a derived value, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value = Some(Source::Derived(Rc::new(value)));

    self
  }

  /// Sets the change handler, run with each new value.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(f32) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }

  /// Binds the value to `signal` in both directions: the slider shows the signal and
  /// each move writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::RwSignal<f32>) -> Self {
    use skene_reactive::{SignalGet, SignalUpdate};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
  }
}

crate::control::control_surface!(Slider);

impl View for Slider {
  fn build(mut self) -> NodeId {
    let style = self.base.style;
    let variants = std::mem::take(&mut self.base.variants);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Slider {
          style,
          variants,
          glyphs: self.glyphs.unwrap_or_else(|| Glyphs::current().slider),
          min: self.min,
          max: self.max,
          step: self.step,
          value: self.min,
        },
        taffy::style::Style::default(),
      )
    });

    if let Some(value) = self.value {
      value.bind(move |value| {
        with_state(|state| {
          state.with_node(node, |record| {
            if let NodeKind::Slider { value: current, .. } = &mut record.kind {
              *current = value;
            }
          });

          state.dirty.set(true);
        });
      });
    }

    if let Some(action) = self.change_handler {
      with_state(|state| {
        state
          .change_value
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
