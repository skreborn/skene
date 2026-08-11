//! A run of text: a color and a bold flag—all a terminal renders.

use std::rc::Rc;

use skene_core::{Color, NodeId, Source, View};

use crate::{
  props::{Condition, RunProp, RunStyle},
  realize::{Binders, split_prop},
  state::NodeKind,
  with_state,
};

/// A run of text.
#[must_use]
pub struct Text {
  source: Source<String>,
  selectable: bool,
  color: Option<Source<Color>>,
  bold: Option<Source<bool>>,
  selection_color: Option<Color>,
  selection_background: Option<Color>,
  variants: Vec<(Condition, RunStyle)>,
}

impl Text {
  /// Creates a static text label.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      source: Source::Static(content.into()),
      selectable: false,
      color: None,
      bold: None,
      selection_color: None,
      selection_background: None,
      variants: Vec::new(),
    }
  }

  /// Creates a derived text label.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      source: Source::Derived(Rc::new(content)),
      selectable: false,
      color: None,
      bold: None,
      selection_color: None,
      selection_background: None,
      variants: Vec::new(),
    }
  }

  /// Sets whether the user can select and copy the text; off by default—selection is
  /// opt-in. Control chrome never selects, this setter notwithstanding.
  #[inline(always)]
  pub const fn set_selectable(mut self, selectable: bool) -> Self {
    self.selectable = selectable;

    self
  }

  /// Sets a static color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.color = Some(Source::Static(color));

    self
  }

  /// Sets a derived color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.color = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets whether the text is bold.
  #[inline(always)]
  pub fn set_bold(mut self, bold: bool) -> Self {
    self.bold = Some(Source::Static(bold));

    self
  }

  /// Sets a derived bold flag.
  #[inline(always)]
  pub fn derive_bold(mut self, bold: impl Fn() -> bool + 'static) -> Self {
    self.bold = Some(Source::Derived(Rc::new(bold)));

    self
  }

  /// Sets the text color while selected, overriding any [`TextTheme`](crate::TextTheme).
  #[inline(always)]
  pub const fn set_selection_color(mut self, color: Color) -> Self {
    self.selection_color = Some(color);

    self
  }

  /// Sets the background behind selected text, overriding any
  /// [`TextTheme`](crate::TextTheme).
  #[inline(always)]
  pub const fn set_selection_background(mut self, color: Color) -> Self {
    self.selection_background = Some(color);

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: RunStyle) -> Self {
    self.variants.push((condition, style));

    self
  }
}

impl View for Text {
  fn build(self) -> NodeId {
    let mut style = RunStyle::new();
    let mut binders = Binders::new();
    let defaults = crate::theme::TextDefaults::current();
    let selectable =
      self.selectable && skene_reactive::use_context::<crate::theme::Unselectable>().is_none();

    style.selection_color = self.selection_color.or(defaults.selection_color);
    style.selection_background = self.selection_background.or(defaults.selection_background);

    split_prop(
      self.color,
      &mut style.color,
      &mut binders,
      RunProp::Color,
      update_run,
    );

    split_prop(
      self.bold,
      &mut style.bold,
      &mut binders,
      RunProp::Bold,
      update_run,
    );

    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Run {
          style,
          variants: self.variants,
          text: String::new(),
          selectable,
        },
        taffy::style::Style::default(),
      )
    });

    for binder in binders {
      binder(node);
    }

    self.source.bind(move |text| set_text(node, &text));

    node
  }
}

/// Applies one changed text-run property.
fn update_run(node: NodeId, prop: RunProp) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Run { style, .. } = &mut record.kind {
        match prop {
          RunProp::Color(value) => style.color = Some(value),
          RunProp::Bold(value) => style.bold = Some(value),
        }
      }
    });

    state.dirty.set(true);
  });
}

/// Sets a run's text content.
fn set_text(node: NodeId, text: &str) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Run { text: current, .. } = &mut record.kind {
        text.clone_into(current);
      }
    });

    state.invalidate_selection(node);
    state.dirty.set(true);
  });
}
