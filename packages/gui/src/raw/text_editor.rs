//! A multi-line text area.

use std::rc::Rc;

use skene_core::Source;

use crate::control::ControlBase;

/// A multi-line text area.
///
/// The value flows both ways: [`derive_value`](Self::derive_value) (or
/// [`set_value`](Self::set_value)) drives the shown text, and
/// [`handle_change`](Self::handle_change) receives each edit in real time—
/// [`bind_value`](Self::bind_value) wires both to one signal. Unstyled by default; the
/// visible height is set in text lines with [`set_lines`](Self::set_lines).
///
/// The unstyled base a component library builds on; [`SimpleTextEditor`](crate::SimpleTextEditor)
/// is the ready-made counterpart.
#[must_use]
pub struct RawTextEditor {
  value: Option<Source<String>>,
  placeholder: Option<String>,
  lines: Option<u16>,
  #[expect(clippy::type_complexity)]
  change_handler: Option<Rc<dyn Fn(&str)>>,
  control: ControlBase,
}

impl RawTextEditor {
  /// Creates an empty text area.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      value: None,
      placeholder: None,
      lines: None,
      change_handler: None,
      control: ControlBase::new(),
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

  /// Sets the visible height, in text lines.
  #[inline(always)]
  pub fn set_lines(mut self, lines: u16) -> Self {
    self.lines = Some(lines);

    self
  }

  /// Sets the change handler, run with the new value on every edit, in real time.
  #[inline(always)]
  pub fn handle_change(mut self, action: impl Fn(&str) + 'static) -> Self {
    self.change_handler = Some(Rc::new(action));

    self
  }
}

impl Default for RawTextEditor {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl RawTextEditor {
  /// Binds the value to `signal` in both directions: the text area shows the signal and
  /// each edit writes it back.
  #[inline(always)]
  pub fn bind_value(self, signal: skene_reactive::Signal<String>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value.to_owned()))
  }
}

impl skene_core::View for RawTextEditor {
  fn build(self) -> skene_core::NodeId {
    use skene_core::TextEditorProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_text_editor(&TextEditorProps {
      placeholder: self.placeholder,
      lines: self.lines,
      control,
    });

    if let Some(value) = self.value {
      value.bind(move |value| driver::set_value(node, &value));
    }

    if let Some(action) = self.change_handler {
      driver::handle_change(node, action);
    }

    handlers.register(node);

    node
  }
}

impl crate::HasEvents for RawTextEditor {
  #[inline(always)]
  fn event_core(&mut self) -> &mut crate::EventCore {
    &mut self.control.events
  }
}

impl crate::HasFocus for RawTextEditor {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    &mut self.control.focus
  }
}

impl crate::HasStyle for RawTextEditor {
  #[inline(always)]
  fn style_core(&mut self) -> &mut crate::StyleCore {
    &mut self.control.style
  }
}
