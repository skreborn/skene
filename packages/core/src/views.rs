//! The built-in widgets.
//!
//! Each widget is its own builder: [`Builder`] derives fluent, self-returning
//! setters for the typed style props that make sense for it (a [`Flex`] has
//! `gap` and axis alignment, a [`Container`] has `padding`/`width`/`background`/…).
//! Because a widget *is* a [`View`], there is no separate builder type and no
//! `.build()` to call — chain setters and pass it straight to a parent.
//!
//! Realizing a widget against the backend happens in [`View::build`], which
//! assembles the relevant subset of the renderer [`Style`] internally — app code
//! never touches `Style` or CSS directly.

use std::{cell::RefCell, rc::Rc};

use skene_macros::Builder;
use skene_reactive::{Effect, Scope};
use skene_renderer::{
  Axis,
  Border,
  Color,
  CrossAxisAlignment,
  Cursor,
  Display,
  EdgeInsets,
  ElementKind,
  MainAxisAlignment,
  NodeId,
  Style,
};

use crate::{
  element::Element,
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
/// Label::new("Hello").font_size(20.0);
/// Label::derived(move || format!("Value: {count}"));
/// ```
#[derive(Builder)]
#[must_use]
pub struct Label {
  source: TextSource,
  /// Text color.
  #[builder(into)]
  color: Option<Color>,
  /// Font size, in logical pixels.
  font_size: Option<f64>,
  /// Font weight (e.g. `400` normal, `700` bold).
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

    let element = Element::new(ElementKind::Text).style(style);
    let element = match self.source {
      TextSource::Static(text) => element.text(text),
      TextSource::Derived(f) => element.text_fn(move || f()),
    };
    element.build()
  }
}

// TODO: Gestures should be abstracted to a core type
// TODO: Button should allow container and label customization

/// A clickable button with text.
///
/// ```ignore
/// Button::new("Increment").action(move || count.update(|c| *c += 1));
/// ```
#[derive(Builder)]
#[must_use]
pub struct Button {
  label: String,
  /// Run a closure when the button is clicked.
  #[builder(callback)]
  action: Option<Rc<dyn Fn()>>,
  /// Background color.
  #[builder(into)]
  background: Option<Color>,
  /// Text color.
  #[builder(into)]
  color: Option<Color>,
  /// Font size, in logical pixels.
  font_size: Option<f64>,
  /// Corner radius, in logical pixels.
  border_radius: Option<f64>,
}

impl Button {
  /// A button labelled with `text`.
  #[inline(always)]
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      label: text.into(),
      action: None,
      background: None,
      color: None,
      font_size: None,
      border_radius: None,
    }
  }
}

impl View for Button {
  fn build(self) -> NodeId {
    // A sensible default look, with per-instance overrides on top.
    let style = Style::new()
      .padding(EdgeInsets::symmetric(20.0, 10.0))
      .font_size(self.font_size.unwrap_or(20.0))
      .background(self.background.unwrap_or(Color::rgb(0x37, 0x6e, 0xff)))
      .color(self.color.unwrap_or(Color::WHITE))
      .border_radius(self.border_radius.unwrap_or(8.0))
      .cursor(Cursor::Pointer);

    let mut element = Element::new(ElementKind::Button)
      .style(style)
      .text(self.label);
    if let Some(action) = self.action {
      element = element.on_click(move || action());
    }
    element.build()
  }
}

/// A flex container holding any number of children.
///
/// Construct with [`Flex::horizontal`] or [`Flex::vertical`] and add children
/// with [`child`](Flex::child):
///
/// ```ignore
/// Flex::horizontal()
///   .gap(16.0)
///   .main_axis_alignment(MainAxisAlignment::Center)
///   .child(Label::new("a"))
///   .child(Label::new("b"));
/// ```
#[derive(Builder)]
#[must_use]
pub struct Flex {
  axis: Axis,
  /// Append a child view.
  #[builder(item = child, view)]
  children: Vec<AnyView>,
  /// Space between children, in logical pixels.
  gap: Option<f64>,
  /// Alignment of children along the main (layout) axis.
  main_axis_alignment: Option<MainAxisAlignment>,
  /// Alignment of children along the cross axis.
  cross_axis_alignment: Option<CrossAxisAlignment>,
  /// Fixed width, in logical pixels.
  width: Option<f64>,
  /// Fixed height, in logical pixels.
  height: Option<f64>,
  /// Inner spacing.
  #[builder(into)]
  padding: Option<EdgeInsets>,
  /// Background color.
  #[builder(into)]
  background: Option<Color>,
  /// A solid border.
  border: Option<Border>,
  /// Corner radius, in logical pixels.
  border_radius: Option<f64>,
}

impl Flex {
  /// A row: children laid out along the horizontal main axis.
  #[inline(always)]
  pub fn horizontal() -> Self {
    Flex::with_axis(Axis::Horizontal)
  }

  /// A column: children laid out along the vertical main axis.
  #[inline(always)]
  pub fn vertical() -> Self {
    Flex::with_axis(Axis::Vertical)
  }

  #[inline(always)]
  fn with_axis(axis: Axis) -> Self {
    Self {
      axis,
      children: Vec::new(),
      gap: None,
      main_axis_alignment: None,
      cross_axis_alignment: None,
      width: None,
      height: None,
      padding: None,
      background: None,
      border: None,
      border_radius: None,
    }
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
    if let Some(v) = self.main_axis_alignment {
      style = style.main_axis_alignment(v);
    }
    if let Some(v) = self.cross_axis_alignment {
      style = style.cross_axis_alignment(v);
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
    if let Some(v) = self.border {
      style = style.border(v);
    }
    if let Some(v) = self.border_radius {
      style = style.border_radius(v);
    }

    Element::new(ElementKind::Box)
      .style(style)
      .children(self.children)
      .build()
  }
}

/// A single-child box with the common container style props.
///
/// ```ignore
/// Container::new(child).padding(16.0).background(Color::WHITE);
/// ```
#[derive(Builder)]
#[must_use]
pub struct Container {
  child: AnyView,
  /// Fixed width, in logical pixels.
  width: Option<f64>,
  /// Fixed height, in logical pixels.
  height: Option<f64>,
  /// Inner spacing.
  #[builder(into)]
  padding: Option<EdgeInsets>,
  /// Background color.
  #[builder(into)]
  background: Option<Color>,
  /// A solid border.
  border: Option<Border>,
  /// Corner radius, in logical pixels.
  border_radius: Option<f64>,
}

impl Container {
  /// A container wrapping `child`.
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      width: None,
      height: None,
      padding: None,
      background: None,
      border: None,
      border_radius: None,
    }
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
    if let Some(v) = self.border {
      style = style.border(v);
    }
    if let Some(v) = self.border_radius {
      style = style.border_radius(v);
    }

    Element::new(ElementKind::Box)
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
