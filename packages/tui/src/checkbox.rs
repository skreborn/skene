//! A toggleable checkbox, in cells.

use std::rc::Rc;

use skene_core::{NodeId, Source, View};

use crate::{control::ControlBase, state::NodeKind, theme::Glyphs, with_state};

/// A checkbox, rendered as a single mark (`☑`/`☐` by default).
///
/// The checked state flows both ways: [`derive_checked`](Self::derive_checked) (or
/// [`set_checked`](Self::set_checked)) drives the mark, and
/// [`handle_toggle`](Self::handle_toggle) receives each flip—
/// [`bind_checked`](Self::bind_checked) wires both to one signal. `Enter`, `Space`, or
/// a mouse click toggles it.
#[must_use]
pub struct Checkbox {
  base: ControlBase,
  checked: Option<Source<bool>>,
  marks: Option<(char, char)>,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
}

impl Checkbox {
  /// Creates an unchecked checkbox.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      checked: None,
      marks: None,
      toggle_handler: None,
    }
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

  /// Sets the checked and unchecked marks, overriding any
  /// [`ControlTheme`](crate::ControlTheme).
  #[inline(always)]
  pub fn set_marks(mut self, checked: char, unchecked: char) -> Self {
    self.marks = Some((checked, unchecked));

    self
  }

  /// Sets the toggle handler, run with the new checked state on each flip.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.toggle_handler = Some(Rc::new(action));

    self
  }

  /// Binds the checked state to `signal` in both directions: the checkbox shows the
  /// signal and each flip writes it back.
  #[inline(always)]
  pub fn bind_checked(self, signal: skene_reactive::RwSignal<bool>) -> Self {
    use skene_reactive::{SignalGet, SignalUpdate};

    self
      .derive_checked(move || signal.get())
      .handle_toggle(move |checked| signal.set(checked))
  }
}

impl Default for Checkbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

crate::control::control_surface!(Checkbox);

impl View for Checkbox {
  fn build(mut self) -> NodeId {
    let style = self.base.style;
    let variants = std::mem::take(&mut self.base.variants);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Checkbox {
          style,
          variants,
          marks: self.marks.unwrap_or_else(|| Glyphs::current().checkbox),
          checked: false,
        },
        taffy::style::Style::default(),
      )
    });

    if let Some(checked) = self.checked {
      checked.bind(move |checked| {
        with_state(|state| {
          state.with_node(node, |record| {
            if let NodeKind::Checkbox {
              checked: current, ..
            } = &mut record.kind
            {
              *current = checked;
            }
          });

          state.dirty.set(true);
        });
      });
    }

    if let Some(action) = self.toggle_handler {
      with_state(|state| {
        state
          .toggle
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
