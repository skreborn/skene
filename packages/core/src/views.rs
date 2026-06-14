//! The built-in widgets.
//!
//! Each widget is a small `bon` builder that exposes only the typed style props
//! that make sense for it (a [`Flex`] has `gap` and axis alignment, a
//! [`Container`] has `padding`/`width`/`background`/…). The builder's `build()`
//! finalizes the widget value; realizing it against the backend happens later
//! through [`View::build`], which assembles the relevant subset of the renderer
//! [`Style`] internally — app code never touches `Style` or CSS directly.

use std::{cell::RefCell, rc::Rc};

use bon::Builder;
use skene_reactive::{Effect, Scope};
use skene_renderer::{
  Axis,
  Color,
  CrossAxisAlignment,
  Cursor,
  Display,
  ElementKind,
  Length,
  MainAxisAlignment,
  NodeId,
  Style,
};

use crate::{
  element::El,
  runtime::with_backend,
  view::{AnyView, IntoView, View},
};

/// The source of a [`Label`]'s text: a fixed string or a reactive closure.
enum TextSource {
  Static(String),
  Derived(Rc<dyn Fn() -> String>),
}

/// A run of text.
///
/// Use [`Label::new`] for static text and [`Label::derived`] for text that
/// re-renders whenever the signals it reads change.
///
/// ```ignore
/// Label::new("Hello").font_size(20.0).build();
/// Label::derived(move || format!("Value: {count}")).build();
/// ```
#[must_use]
pub struct Label {
  source: TextSource,
  color: Option<Color>,
  font_size: Option<Length>,
  font_weight: Option<u16>,
}

impl Label {
  /// A label with fixed text.
  #[inline(always)]
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      source: TextSource::Static(text.into()),
      color: None,
      font_size: None,
      font_weight: None,
    }
  }

  /// A label whose text is recomputed reactively from `f`.
  #[inline(always)]
  pub fn derived(f: impl Fn() -> String + 'static) -> Self {
    Self {
      source: TextSource::Derived(Rc::new(f)),
      color: None,
      font_size: None,
      font_weight: None,
    }
  }

  pub fn color(mut self, color: impl Into<Color>) -> Self {
    self.color = Some(color.into());

    self
  }

  pub fn font_size(mut self, font_size: impl Into<Length>) -> Self {
    self.font_size = Some(font_size.into());

    self
  }

  pub fn font_weight(mut self, font_weight: u16) -> Self {
    self.font_weight = Some(font_weight);

    self
  }
}

impl View for Label {
  fn build(self) -> NodeId {
    let mut style = Style::new();
    if let Some(v) = self.color {
      style = style.color(v);
    }
    if let Some(v) = self.font_size {
      style = style.font_size(v);
    }
    if let Some(v) = self.font_weight {
      style = style.font_weight(v);
    }

    let el = El::new(ElementKind::Text).style(style);
    let el = match self.source {
      TextSource::Static(text) => el.text(text),
      TextSource::Derived(f) => el.text_fn(move || f()),
    };
    el.build()
  }
}

/// A clickable button with text.
///
/// ```ignore
/// Button::new("Increment").action(move || count.update(|c| *c += 1)).build();
/// ```
#[derive(Builder)]
#[builder(start_fn = with_label)]
#[must_use]
pub struct Button {
  #[builder(start_fn, into)]
  label: String,
  #[builder(field)]
  action: Option<Rc<dyn Fn()>>,
  background: Option<Color>,
  color: Option<Color>,
  #[builder(into)]
  font_size: Option<Length>,
  #[builder(into)]
  border_radius: Option<Length>,
}

impl Button {
  /// A button labelled with `text`.
  #[inline(always)]
  #[allow(clippy::new_ret_no_self)] // returns a builder; `build()` yields the `Button`
  pub fn new(text: impl Into<String>) -> ButtonBuilder {
    Button::with_label(text.into())
  }
}

impl<S: button_builder::State> ButtonBuilder<S> {
  /// Run `f` when the button is clicked.
  #[inline(always)]
  pub fn action(mut self, f: impl Fn() + 'static) -> Self {
    self.action = Some(Rc::new(f));
    self
  }
}

impl View for Button {
  fn build(self) -> NodeId {
    // A sensible default look, with per-instance overrides on top.
    let style = Style::new()
      .padding_horiz(20.0)
      .padding_vert(10.0)
      .font_size(self.font_size.unwrap_or(Length::Px(20.0)))
      .background(self.background.unwrap_or(Color::rgb(0x37, 0x6e, 0xff)))
      .color(self.color.unwrap_or(Color::WHITE))
      .border_radius(self.border_radius.unwrap_or(Length::Px(8.0)))
      .cursor(Cursor::Pointer);

    let mut el = El::new(ElementKind::Button).style(style).text(self.label);
    if let Some(action) = self.action {
      el = el.on_click(move || action());
    }
    el.build()
  }
}

/// A flex container holding any number of children.
///
/// Construct with [`Flex::horizontal`] or [`Flex::vertical`] and add children
/// with [`child`](FlexBuilder::child):
///
/// ```ignore
/// Flex::horizontal()
///   .gap(16.0)
///   .main_axis_align(MainAxisAlignment::Center)
///   .child(Label::new("a").build())
///   .child(Label::new("b").build())
///   .build();
/// ```
#[derive(Builder)]
#[builder(start_fn = new)]
#[must_use]
pub struct Flex {
  #[builder(start_fn)]
  axis: Axis,
  #[builder(field)]
  children: Vec<AnyView>,
  #[builder(into)]
  gap: Option<Length>,
  main_axis_align: Option<MainAxisAlignment>,
  cross_axis_align: Option<CrossAxisAlignment>,
  #[builder(into)]
  width: Option<Length>,
  #[builder(into)]
  height: Option<Length>,
  #[builder(into)]
  padding: Option<Length>,
  background: Option<Color>,
  #[builder(into)]
  border_radius: Option<Length>,
}

impl Flex {
  /// A row: children laid out along the horizontal main axis.
  #[inline(always)]
  pub fn horizontal() -> FlexBuilder {
    Flex::new(Axis::Horizontal)
  }

  /// A column: children laid out along the vertical main axis.
  #[inline(always)]
  pub fn vertical() -> FlexBuilder {
    Flex::new(Axis::Vertical)
  }
}

impl<S: flex_builder::State> FlexBuilder<S> {
  /// Append a child.
  #[inline(always)]
  pub fn child(mut self, child: impl IntoView) -> Self {
    self.children.push(child.into_view());
    self
  }

  /// Append several children.
  #[inline(always)]
  pub fn children(mut self, children: impl IntoIterator<Item = AnyView>) -> Self {
    self.children.extend(children);
    self
  }
}

impl View for Flex {
  fn build(self) -> NodeId {
    let mut style = Style::new()
      .display(Display::Flex)
      .flex_direction(self.axis);

    if let Some(v) = self.gap {
      style = style.gap(v);
    }
    if let Some(v) = self.main_axis_align {
      style = style.main_axis_align(v);
    }
    if let Some(v) = self.cross_axis_align {
      style = style.cross_axis_align(v);
    }
    if let Some(v) = self.width {
      style = style.width(v);
    }
    if let Some(v) = self.height {
      style = style.height(v);
    }
    if let Some(v) = self.padding {
      style = style.padding(v);
    }
    if let Some(v) = self.background {
      style = style.background(v);
    }
    if let Some(v) = self.border_radius {
      style = style.border_radius(v);
    }

    El::new(ElementKind::Box)
      .style(style)
      .children(self.children)
      .build()
  }
}

/// A single-child box with the common container style props.
///
/// ```ignore
/// Container::new(child).padding(16.0).background(Color::WHITE).build();
/// ```
#[derive(Builder)]
#[builder(start_fn = with_child)]
#[must_use]
pub struct Container {
  #[builder(start_fn)]
  child: AnyView,
  #[builder(into)]
  width: Option<Length>,
  #[builder(into)]
  height: Option<Length>,
  #[builder(into)]
  padding: Option<Length>,
  background: Option<Color>,
  #[builder(into)]
  border_radius: Option<Length>,
}

impl Container {
  /// A container wrapping `child`.
  #[inline(always)]
  #[allow(clippy::new_ret_no_self)] // returns a builder; `build()` yields the `Container`
  pub fn new(child: impl IntoView) -> ContainerBuilder {
    Container::with_child(child.into_view())
  }
}

impl View for Container {
  fn build(self) -> NodeId {
    let mut style = Style::new();
    if let Some(v) = self.width {
      style = style.width(v);
    }
    if let Some(v) = self.height {
      style = style.height(v);
    }
    if let Some(v) = self.padding {
      style = style.padding(v);
    }
    if let Some(v) = self.background {
      style = style.background(v);
    }
    if let Some(v) = self.border_radius {
      style = style.border_radius(v);
    }

    El::new(ElementKind::Box)
      .style(style)
      .child_any(self.child)
      .build()
  }
}

/// A reactive subtree: `f` is re-run whenever signals it reads change, and the
/// previously built subtree (and everything its scope owns) is disposed.
#[inline(always)]
pub fn dyn_view(f: impl Fn() -> AnyView + 'static) -> DynView {
  DynView { f: Box::new(f) }
}

#[must_use]
pub struct DynView {
  f: Box<dyn Fn() -> AnyView>,
}

impl View for DynView {
  fn build(self) -> NodeId {
    let container = with_backend(|b| b.create_element(ElementKind::Box));
    let f = self.f;
    let parent_scope = Scope::current();
    // Holds the currently mounted child node and the scope that owns its effects.
    let current = Rc::new(RefCell::new(None::<(NodeId, Scope)>));

    Effect::new(move |_| {
      let child_scope = parent_scope.create_child();
      let new_node = child_scope.enter(|| (f)().build());

      let previous = current.borrow_mut().take();
      with_backend(|b| {
        b.insert_child(container, new_node, None);
        if let Some((old_node, _)) = &previous {
          b.remove_child(container, *old_node);
          b.destroy(*old_node);
        }
      });
      if let Some((_, old_scope)) = previous {
        old_scope.dispose();
      }
      *current.borrow_mut() = Some((new_node, child_scope));
    });

    container
  }
}
