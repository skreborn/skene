use std::rc::Rc;

use skene_reactive::Effect;
use skene_renderer::{ElementKind, Event, EventKind, NodeId, Style};

use crate::{
  runtime::{Handler, register_handler, with_backend},
  view::{AnyView, IntoView, View},
};

/// The core element builder.
///
/// Almost every built-in view is an `El` under the hood. It collects a node's
/// kind, style (static and/or reactive), text, attributes, children and event
/// handlers, and realizes them all in [`View::build`].
#[must_use]
pub struct El {
  kind: ElementKind,
  base_style: Option<Style>,
  style_fn: Option<Box<dyn Fn() -> Style>>,
  text: Option<String>,
  text_fn: Option<Box<dyn Fn() -> String>>,
  attrs: Vec<(String, Option<String>)>,
  handlers: Vec<(EventKind, Handler)>,
  children: Vec<AnyView>,
}

impl El {
  #[inline(always)]
  pub fn new(kind: ElementKind) -> Self {
    El {
      kind,
      base_style: None,
      style_fn: None,
      text: None,
      text_fn: None,
      attrs: Vec::new(),
      handlers: Vec::new(),
      children: Vec::new(),
    }
  }

  /// Apply a static style. Repeated calls merge, later winning.
  #[inline(always)]
  pub fn style(mut self, style: Style) -> Self {
    self.base_style = Some(match self.base_style.take() {
      Some(base) => base.merge(&style),
      None => style,
    });
    self
  }

  /// Apply a reactive style: re-applied whenever its tracked signals change.
  /// Merged on top of any static style.
  #[inline(always)]
  pub fn style_fn(mut self, f: impl Fn() -> Style + 'static) -> Self {
    self.style_fn = Some(Box::new(f));
    self
  }

  /// Set static text content.
  #[inline(always)]
  pub fn text(mut self, text: impl Into<String>) -> Self {
    self.text = Some(text.into());
    self
  }

  /// Set reactive text content: re-applied whenever its tracked signals change.
  #[inline(always)]
  pub fn text_fn(mut self, f: impl Fn() -> String + 'static) -> Self {
    self.text_fn = Some(Box::new(f));
    self
  }

  #[inline(always)]
  pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
    self.attrs.push((name.into(), Some(value.into())));
    self
  }

  /// Subscribe to an event category.
  #[inline(always)]
  pub fn on_event(mut self, kind: EventKind, handler: impl Fn(Event) + 'static) -> Self {
    self.handlers.push((kind, Rc::new(handler)));
    self
  }

  /// Convenience: handle clicks (the event payload is ignored).
  #[inline(always)]
  pub fn on_click(self, handler: impl Fn() + 'static) -> Self {
    self.on_event(EventKind::Click, move |_| handler())
  }

  #[inline(always)]
  pub fn child(mut self, child: impl IntoView) -> Self {
    self.children.push(child.into_view());
    self
  }

  #[inline(always)]
  pub fn child_any(mut self, child: AnyView) -> Self {
    self.children.push(child);
    self
  }

  #[inline(always)]
  pub fn children(mut self, children: impl IntoIterator<Item = AnyView>) -> Self {
    self.children.extend(children);
    self
  }
}

impl View for El {
  fn build(self) -> NodeId {
    let node = with_backend(|b| b.create_element(self.kind));

    // Style: a reactive style re-applies on change, merged over the static base.
    match (self.base_style, self.style_fn) {
      (base, Some(f)) => {
        Effect::new(move |_| {
          let style = match &base {
            Some(base) => base.clone().merge(&f()),
            None => f(),
          };
          with_backend(|b| b.set_style(node, &style));
        });
      }
      (Some(base), None) => {
        with_backend(|b| b.set_style(node, &base));
      }
      (None, None) => {}
    }

    // Text.
    match (self.text, self.text_fn) {
      (_, Some(f)) => {
        Effect::new(move |_| {
          let text = f();
          with_backend(|b| b.set_text(node, &text));
        });
      }
      (Some(text), None) => {
        with_backend(|b| b.set_text(node, &text));
      }
      (None, None) => {}
    }

    for (name, value) in self.attrs {
      with_backend(|b| b.set_attribute(node, &name, value.as_deref()));
    }

    for (kind, handler) in self.handlers {
      register_handler(node, kind, handler);
    }

    for child in self.children {
      let child_node = child.build();
      with_backend(|b| b.insert_child(node, child_node, None));
    }

    node
  }
}
