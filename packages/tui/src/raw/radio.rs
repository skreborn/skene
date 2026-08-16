//! One choice in a radio group, in cells.

use std::rc::Rc;

use skene_core::{NodeId, Prevent, Source, View};

use crate::{
  control::{Behavior, ControlBase},
  state::NodeKind,
  theme::Glyphs,
  with_state,
};

/// A radio button, rendered as a single mark (`◉`/`○` by default); radios sharing a
/// group are exclusive.
///
/// Checking one radio unchecks its group. `Enter`, `Space`, or a mouse click checks the
/// focused radio; arrow keys move the check through the group.
/// [`bind_value`](Self::bind_value) wires a whole group to one signal holding the
/// checked radio's value.
///
/// The unstyled base a component library builds on; [`SimpleRadio`](crate::SimpleRadio) is
/// the ready-made counterpart.
#[must_use]
pub struct RawRadio {
  base: ControlBase<crate::props::ControlStyle>,
  group: String,
  value: String,
  checked: Option<Source<bool>>,
  marks: Option<(String, String)>,
  label: Option<String>,
  toggle_handler: Option<Rc<dyn Fn(bool)>>,
}

impl RawRadio {
  /// Creates an unchecked radio carrying `value` within `group`.
  #[inline(always)]
  pub fn new(group: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      base: ControlBase::new(),
      group: group.into(),
      value: value.into(),
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

  /// Sets the label after the mark; clicking it checks too.
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets the toggle handler, run with the new checked state as the check moves.
  #[inline(always)]
  pub fn handle_toggle(mut self, action: impl Fn(bool) + 'static) -> Self {
    self.toggle_handler = Some(Rc::new(action));

    self
  }

  /// Binds the radio to `signal` in both directions: it shows checked while the signal
  /// holds its value, and checking it writes its value back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Set, With};

    let value = self.value.clone();
    let chosen = self.value.clone();

    self
      .derive_checked(move || signal.with(|current| *current == value))
      .handle_toggle(move |checked| {
        if checked {
          signal.set(chosen.clone());
        }
      })
  }
}

impl View for RawRadio {
  fn build(mut self) -> NodeId {
    let style = self.base.style.style;
    let variants = std::mem::take(&mut self.base.style.variants);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Radio {
          style,
          variants,
          group: self.group,
          marks: self.marks.unwrap_or_else(|| Glyphs::current().radio),
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
            if let NodeKind::Radio {
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

impl crate::HasEvents for RawRadio {
  #[inline(always)]
  fn event_core(&mut self) -> &mut crate::EventCore {
    &mut self.base.events
  }
}

impl crate::HasFocus for RawRadio {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    &mut self.base.focus
  }
}

impl crate::HasMarks for RawRadio {
  #[inline(always)]
  fn mark_core(&mut self) -> &mut crate::MarkCore {
    &mut self.base.style
  }
}
