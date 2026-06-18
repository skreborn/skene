//! The web backend's styled text stack: [`Text`], [`TextTheme`], [`Span`] and
//! [`RichText`]. Text inheritance is resolved by the framework as recorded
//! appliers rather than by browser behavior.

use std::rc::Rc;

use skene_core::{AnyView, Backend, Color, IntoView, NodeId, Number, Source, View};
use skene_reactive::{Context, Scope};

use crate::{WebBackend, with_backend};

// TODO: Font weight should be an enum that accepts a number or a keyword.

/// The text style of a text node: a bag of optional properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct TextStyle {
  /// The text color.
  pub color: Option<Color>,
  /// The font size.
  pub font_size: Option<Number>,
  /// The font weight.
  pub font_weight: Option<Number>,
}

impl TextStyle {
  /// Creates an empty text style.
  #[inline(always)]
  pub fn new() -> Self {
    TextStyle::default()
  }

  /// Sets the text color.
  #[inline(always)]
  pub fn set_color(mut self, value: Color) -> Self {
    self.color = Some(value);

    self
  }

  /// Sets the font size.
  #[inline(always)]
  pub fn set_font_size(mut self, value: impl Into<Number>) -> Self {
    self.font_size = Some(value.into());

    self
  }

  /// Sets the font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, value: impl Into<Number>) -> Self {
    self.font_weight = Some(value.into());

    self
  }
}

/// A closure that applies one styling property to a node.
type Applier = Rc<dyn Fn(NodeId)>;

/// Text styling inherited by a subtree, as recorded appliers.
#[derive(Clone, Default)]
struct InheritedText {
  base: Vec<Applier>,
  anchor: Vec<Applier>,
}

impl TextStyle {
  /// Whether the style sets no properties.
  #[inline(always)]
  fn is_empty(&self) -> bool {
    self.color.is_none() && self.font_size.is_none() && self.font_weight.is_none()
  }

  /// A single applier that sets every property this style holds in one intern, or
  /// none if the style is empty.
  fn appliers(&self) -> Vec<Applier> {
    if self.is_empty() {
      return Vec::new();
    }

    let style = *self;

    vec![Rc::new(move |node| {
      with_backend(|backend| {
        backend.set_text_style(node, style.color, style.font_size, style.font_weight);
      });
    })]
  }
}

/// Derived (reactive) text style: a closure per property, bound to a node so the
/// property updates when its source changes.
///
/// Closures are `Rc` so the style is cloneable, letting a group span hand the same
/// derived style to each descendant.
#[derive(Clone, Default)]
struct DerivedTextStyle {
  color: Option<Rc<dyn Fn() -> Color>>,
  font_size: Option<Rc<dyn Fn() -> Number>>,
  font_weight: Option<Rc<dyn Fn() -> Number>>,
}

impl DerivedTextStyle {
  /// Whether the style sets no properties.
  #[inline(always)]
  fn is_empty(&self) -> bool {
    self.color.is_none() && self.font_size.is_none() && self.font_weight.is_none()
  }

  /// Binds each property this style holds to `node`.
  fn apply(&self, node: NodeId) {
    if let Some(color) = &self.color {
      let color = Rc::clone(color);

      Source::Derived(Box::new(move || color()))
        .bind(move |value| with_backend(|backend| backend.set_text_color(node, value)));
    }

    if let Some(font_size) = &self.font_size {
      let font_size = Rc::clone(font_size);

      Source::Derived(Box::new(move || font_size()))
        .bind(move |value| with_backend(|backend| backend.set_font_size(node, value)));
    }

    if let Some(font_weight) = &self.font_weight {
      let font_weight = Rc::clone(font_weight);

      Source::Derived(Box::new(move || font_weight()))
        .bind(move |value| with_backend(|backend| backend.set_font_weight(node, value)));
    }
  }

  /// An applier that binds this style, or none if it is empty. Used to inherit a
  /// group span's derived style onto each descendant.
  fn appliers(&self) -> Vec<Applier> {
    if self.is_empty() {
      return Vec::new();
    }

    let style = self.clone();

    vec![Rc::new(move |node| style.apply(node))]
  }
}

/// A run of text.
#[must_use]
pub struct Text {
  source: Source<String>,
  create: Option<Box<dyn FnOnce() -> NodeId>>,
  style: TextStyle,
  derived: DerivedTextStyle,
}

impl Text {
  /// Creates a static text label.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      source: Source::Static(content.into()),
      create: None,
      style: TextStyle::new(),
      derived: DerivedTextStyle::default(),
    }
  }

  /// Creates a derived text label.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      source: Source::Derived(Box::new(content)),
      create: None,
      style: TextStyle::new(),
      derived: DerivedTextStyle::default(),
    }
  }

  /// Makes the text a link to the given destination.
  #[inline(always)]
  pub fn set_anchor(mut self, destination: impl Into<String>) -> Self {
    let destination = destination.into();

    self.create = Some(Box::new(move || {
      with_backend(|backend| backend.create_anchor(&destination))
    }));

    self
  }

  /// Sets a static color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.style = self.style.set_color(color);

    self
  }

  /// Sets a derived color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.derived.color = Some(Rc::new(color));

    self
  }

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: impl Into<Number>) -> Self {
    self.style = self.style.set_font_size(font_size);

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> Number + 'static) -> Self {
    self.derived.font_size = Some(Rc::new(font_size));

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: impl Into<Number>) -> Self {
    self.style = self.style.set_font_weight(font_weight);

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> Number + 'static) -> Self {
    self.derived.font_weight = Some(Rc::new(font_weight));

    self
  }
}

impl View for Text {
  fn build(self) -> NodeId {
    let is_anchor = self.create.is_some();

    let node = match self.create {
      Some(create) => create(),
      None => with_backend(WebBackend::create_text),
    };

    with_backend(|backend| backend.set_text_block(node));

    let inherited = Context::get::<InheritedText>().unwrap_or_default();

    for apply in &inherited.base {
      apply(node);
    }

    if is_anchor {
      for apply in &inherited.anchor {
        apply(node);
      }
    }

    for apply in self.style.appliers() {
      apply(node);
    }

    self.derived.apply(node);

    self
      .source
      .bind(move |text| with_backend(|backend| backend.set_text(node, &text)));

    node
  }
}

/// Overrides the inherited text style for its subtree.
///
/// `base` is inherited by every descendant; `anchor` overrides links within the
/// subtree, layered under a link's own overrides. Nested themes accumulate.
#[must_use]
pub struct TextTheme {
  child: AnyView,
  base: Vec<Applier>,
  anchor: Vec<Applier>,
}

impl TextTheme {
  /// Creates a text theme wrapping `child`.
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      base: Vec::new(),
      anchor: Vec::new(),
    }
  }

  /// Sets the base text style inherited by the subtree.
  #[inline(always)]
  pub fn set_base(mut self, style: TextStyle) -> Self {
    self.base.extend(style.appliers());

    self
  }

  /// Overrides the default style of links in the subtree.
  #[inline(always)]
  pub fn set_anchor(mut self, style: TextStyle) -> Self {
    self.anchor.extend(style.appliers());

    self
  }
}

impl View for TextTheme {
  fn build(self) -> NodeId {
    let mut inherited = Context::get::<InheritedText>().unwrap_or_default();

    inherited.base.extend(self.base);
    inherited.anchor.extend(self.anchor);

    let scope = Scope::current().create_child();

    scope.enter(|| {
      Context::provide(inherited);

      self.child.build()
    })
  }
}

/// A run within a [`RichText`]: either text or a group of nested spans.
#[must_use]
pub struct Span {
  content: SpanContent,
  create: Option<Box<dyn FnOnce() -> NodeId>>,
  style: TextStyle,
  derived: DerivedTextStyle,
}

enum SpanContent {
  Text(Source<String>),
  Group(Vec<Span>),
  View(AnyView),
}

impl Span {
  /// A run of text.
  #[inline(always)]
  pub fn text(content: impl Into<String>) -> Self {
    Span::with_content(SpanContent::Text(Source::Static(content.into())))
  }

  /// A derived run of text.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Span::with_content(SpanContent::Text(Source::Derived(Box::new(content))))
  }

  /// A group of nested spans, styled as a unit.
  #[inline(always)]
  pub fn group(children: impl IntoIterator<Item = Span>) -> Self {
    Span::with_content(SpanContent::Group(children.into_iter().collect()))
  }

  /// An arbitrary view embedded inline within the run, laid out as an inline-level
  /// block box among the surrounding text.
  #[inline(always)]
  pub fn view(view: impl IntoView) -> Self {
    Span::with_content(SpanContent::View(view.into_view()))
  }

  #[inline(always)]
  fn with_content(content: SpanContent) -> Self {
    Self {
      content,
      create: None,
      style: TextStyle::new(),
      derived: DerivedTextStyle::default(),
    }
  }

  /// Makes the span a link to the given destination.
  #[inline(always)]
  pub fn set_anchor(mut self, destination: impl Into<String>) -> Self {
    let destination = destination.into();

    self.create = Some(Box::new(move || {
      with_backend(|backend| backend.create_anchor(&destination))
    }));

    self
  }

  /// Sets a static color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.style = self.style.set_color(color);

    self
  }

  /// Sets a derived color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.derived.color = Some(Rc::new(color));

    self
  }

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: impl Into<Number>) -> Self {
    self.style = self.style.set_font_size(font_size);

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> Number + 'static) -> Self {
    self.derived.font_size = Some(Rc::new(font_size));

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: impl Into<Number>) -> Self {
    self.style = self.style.set_font_weight(font_weight);

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> Number + 'static) -> Self {
    self.derived.font_weight = Some(Rc::new(font_weight));

    self
  }

  fn build(self) -> NodeId {
    let is_anchor = self.create.is_some();

    let node = match self.create {
      Some(create) => create(),
      None => with_backend(WebBackend::create_text),
    };

    let inherited = Context::get::<InheritedText>().unwrap_or_default();

    for apply in &inherited.base {
      apply(node);
    }

    if is_anchor {
      for apply in &inherited.anchor {
        apply(node);
      }
    }

    for apply in self.style.appliers() {
      apply(node);
    }

    self.derived.apply(node);

    match self.content {
      SpanContent::Text(source) => {
        source.bind(move |text| with_backend(|backend| backend.set_text(node, &text)));
      }
      SpanContent::Group(children) => {
        let mut child_inherited = inherited;

        child_inherited.base.extend(self.style.appliers());
        child_inherited.base.extend(self.derived.appliers());

        let scope = Scope::current().create_child();

        scope.enter(|| {
          Context::provide(child_inherited);

          for child in children {
            let child = child.build();

            with_backend(|backend| backend.attach_child(node, child, None));
          }
        });
      }
      SpanContent::View(view) => {
        with_backend(|backend| backend.set_inline_block(node));

        let child = view.build();

        with_backend(|backend| backend.attach_child(node, child, None));
      }
    }

    node
  }
}

/// A sequence of [`Span`]s laid out inline, so styling can vary within one run.
#[must_use]
pub struct RichText {
  spans: Vec<Span>,
}

impl RichText {
  /// Creates rich text from a sequence of spans.
  #[inline(always)]
  pub fn new(spans: impl IntoIterator<Item = Span>) -> Self {
    Self {
      spans: spans.into_iter().collect(),
    }
  }
}

impl View for RichText {
  fn build(self) -> NodeId {
    let node = with_backend(WebBackend::create_text);

    with_backend(|backend| backend.set_text_block(node));

    for span in self.spans {
      let child = span.build();

      with_backend(|backend| backend.attach_child(node, child, None));
    }

    node
  }
}
