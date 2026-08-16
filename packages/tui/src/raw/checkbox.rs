//! A toggleable checkbox, in cells.

use std::rc::Rc;

use skene_core::{NodeId, Prevent, Source, View};

use crate::{
  control::{Behavior, ControlBase},
  state::NodeKind,
  theme::Glyphs,
  with_state,
};

/// A checkbox: its mark (`[x]`/`[ ]` by default), then the optional label—one
/// control, so clicking the label toggles too.
///
/// The checked state flows both ways: [`derive_checked`](Self::derive_checked) (or
/// [`set_checked`](Self::set_checked)) drives the mark, and
/// [`handle_toggle`](Self::handle_toggle) receives each flip—
/// [`bind_checked`](Self::bind_checked) wires both to one signal. `Enter`, `Space`, or
/// a mouse click toggles it.
///
/// The unstyled base a component library builds on; [`SimpleCheckbox`](crate::SimpleCheckbox) is
/// the ready-made counterpart.
#[must_use]
pub struct RawCheckbox {
  base: ControlBase<crate::props::ControlStyle>,
  checked: Option<Source<bool>>,
  marks: Option<(String, String)>,
  label: Option<String>,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
}

impl RawCheckbox {
  /// Creates an unchecked checkbox.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      checked: None,
      marks: None,
      label: None,
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
  pub fn set_marks(mut self, checked: impl Into<String>, unchecked: impl Into<String>) -> Self {
    self.marks = Some((checked.into(), unchecked.into()));

    self
  }

  /// Sets the label after the mark; clicking it toggles too.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

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
  pub fn bind_checked(self, signal: skene_reactive::Signal<bool>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_checked(move || signal.get())
      .handle_toggle(move |checked| signal.set(checked))
  }
}

impl Default for RawCheckbox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for RawCheckbox {
  fn build(mut self) -> NodeId {
    let style = self.base.style.style;
    let variants = std::mem::take(&mut self.base.style.variants);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Checkbox {
          style,
          variants,
          marks: self.marks.unwrap_or_else(|| Glyphs::current().checkbox),
          label: self.label,
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

    self
      .base
      .register(node, Behavior::new().click(|| Prevent::Bubbling));

    node
  }
}

impl crate::HasEvents for RawCheckbox {
  #[inline(always)]
  fn event_core(&mut self) -> &mut crate::EventCore {
    &mut self.base.events
  }
}

impl crate::HasFocus for RawCheckbox {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    &mut self.base.focus
  }
}

impl crate::HasMarks for RawCheckbox {
  #[inline(always)]
  fn mark_core(&mut self) -> &mut crate::MarkCore {
    &mut self.base.style
  }
}
