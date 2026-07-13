// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::{AnyView, Color, IntoView, NodeId, Source, View};
use skene_reactive::{Context, Scope};

/// The text style of a text node: an optional static-or-reactive [`Source`] per property.
#[derive(Clone, Default)]
#[must_use]
pub struct TextStyle {
  /// The text color.
  pub color: Option<Source<Color>>,
  /// The font size, in logical pixels.
  pub font_size: Option<Source<f32>>,
  /// The font weight.
  pub font_weight: Option<Source<f32>>,
}

impl TextStyle {
  /// Creates an empty text style.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
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

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: f32) -> Self {
    self.font_size = Some(Source::Static(font_size));

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> f32 + 'static) -> Self {
    self.font_size = Some(Source::Derived(Rc::new(font_size)));

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: f32) -> Self {
    self.font_weight = Some(Source::Static(font_weight));

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> f32 + 'static) -> Self {
    self.font_weight = Some(Source::Derived(Rc::new(font_weight)));

    self
  }

  /// Overlays each set property of `over` onto `self`, `over` winning.
  #[inline(always)]
  fn overlay(self, over: Self) -> Self {
    Self {
      color: over.color.or(self.color),
      font_size: over.font_size.or(self.font_size),
      font_weight: over.font_weight.or(self.font_weight),
    }
  }
}

/// A run of text.
#[must_use]
pub struct Text {
  pub(crate) source: Source<String>,
  pub(crate) link: Option<String>,
  pub(crate) style: TextStyle,
}

impl Text {
  /// Creates a static text label.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      source: Source::Static(content.into()),
      link: None,
      style: TextStyle::new(),
    }
  }

  /// Creates a derived text label.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      source: Source::Derived(Rc::new(content)),
      link: None,
      style: TextStyle::new(),
    }
  }

  /// Makes the text a link to the given destination.
  #[inline(always)]
  pub fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.link = Some(destination.into());

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
    self.style = self.style.derive_color(color);

    self
  }

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: f32) -> Self {
    self.style = self.style.set_font_size(font_size);

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_size(font_size);

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: f32) -> Self {
    self.style = self.style.set_font_weight(font_weight);

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_weight(font_weight);

    self
  }
}

/// An inline run within a [`RichText`]: text, a group of nested spans styled as a unit, or an
/// arbitrary view embedded among the text.
///
/// Spans flow together and wrap as one line, so styling can vary within it — unlike a [`Text`],
/// which is its own block. A [group](Self::group)'s style cascades to its children; a
/// [view](Self::view) is laid out as an inline-level box among the surrounding runs.
#[must_use]
pub struct Span {
  pub(crate) content: SpanContent,
  pub(crate) link: Option<String>,
  pub(crate) style: TextStyle,
}

/// What a [`Span`] renders: a text run, nested spans, or an embedded view.
pub(crate) enum SpanContent {
  Text(Source<String>),
  Group(Vec<Span>),
  View(AnyView),
}

impl Span {
  /// A static run of text.
  #[inline(always)]
  pub fn text(content: impl Into<String>) -> Self {
    Self::with_content(SpanContent::Text(Source::Static(content.into())))
  }

  /// A derived run of text.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self::with_content(SpanContent::Text(Source::Derived(Rc::new(content))))
  }

  /// A group of nested spans, styled as a unit — the group's style cascades to its children.
  #[inline(always)]
  pub fn group(children: impl IntoIterator<Item = Span>) -> Self {
    Self::with_content(SpanContent::Group(children.into_iter().collect()))
  }

  /// An arbitrary view embedded inline within the run, laid out as an inline-level box among
  /// the surrounding text.
  #[inline(always)]
  pub fn view(view: impl IntoView) -> Self {
    Self::with_content(SpanContent::View(view.into_view()))
  }

  #[inline(always)]
  fn with_content(content: SpanContent) -> Self {
    Self {
      content,
      link: None,
      style: TextStyle::new(),
    }
  }

  /// Makes the run a link to the given destination.
  #[inline(always)]
  pub fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.link = Some(destination.into());

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
    self.style = self.style.derive_color(color);

    self
  }

  /// Sets a static font size.
  #[inline(always)]
  pub fn set_font_size(mut self, font_size: f32) -> Self {
    self.style = self.style.set_font_size(font_size);

    self
  }

  /// Sets a derived font size.
  #[inline(always)]
  pub fn derive_font_size(mut self, font_size: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_size(font_size);

    self
  }

  /// Sets a static font weight.
  #[inline(always)]
  pub fn set_font_weight(mut self, font_weight: f32) -> Self {
    self.style = self.style.set_font_weight(font_weight);

    self
  }

  /// Sets a derived font weight.
  #[inline(always)]
  pub fn derive_font_weight(mut self, font_weight: impl Fn() -> f32 + 'static) -> Self {
    self.style = self.style.derive_font_weight(font_weight);

    self
  }
}

/// A line of text built from inline [`Span`]s, which flow together and wrap so styling can vary
/// within one line.
#[must_use]
pub struct RichText {
  pub(crate) spans: Vec<Span>,
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

/// Provides `base` as the inherited text style for the spans built in `build`, cascading a
/// group's resolved style to its nested children.
#[inline(always)]
pub(crate) fn provide_group_base(base: TextStyle, build: impl FnOnce()) {
  let mut inherited = Context::get::<InheritedText>().unwrap_or_default();

  inherited.base = base;

  let scope = Scope::current().create_child();

  scope.enter(|| {
    Context::provide(inherited);

    build();
  });
}

/// Overrides the inherited text style for its subtree.
///
/// The base style is inherited by every descendant; the `link` style overrides links
/// within the subtree, layered under a link's own overrides. Each style is a full
/// [`TextStyle`], so it can carry static and derived properties alike. Nested themes
/// accumulate.
#[must_use]
pub struct TextTheme {
  pub(crate) child: AnyView,
  pub(crate) base: TextStyle,
  pub(crate) link: TextStyle,
}

impl TextTheme {
  /// Creates a text theme wrapping `child`.
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      base: TextStyle::new(),
      link: TextStyle::new(),
    }
  }

  /// Sets the base text style inherited by the subtree.
  #[inline(always)]
  pub fn set_base(mut self, style: TextStyle) -> Self {
    self.base = style;

    self
  }

  /// Overrides the default style of links in the subtree.
  #[inline(always)]
  pub fn set_link(mut self, style: TextStyle) -> Self {
    self.link = style;

    self
  }
}

/// Text styling inherited by a subtree, threaded through the reactive `Context`.
#[derive(Clone, Default)]
struct InheritedText {
  base: TextStyle,
  link: TextStyle,
}

/// Resolves a text node's effective style from the inherited theme (via `Context`) and the
/// node's own style. Platform-agnostic; a platform realizes the result. `is_link` layers the
/// theme's link style under the node's own.
#[inline(always)]
pub(crate) fn resolved_text_style(style: TextStyle, is_link: bool) -> TextStyle {
  let inherited = Context::get::<InheritedText>().unwrap_or_default();
  let mut resolved = inherited.base;

  if is_link {
    resolved = resolved.overlay(inherited.link);
  }

  resolved.overlay(style)
}

impl View for TextTheme {
  fn build(self) -> NodeId {
    let mut inherited = Context::get::<InheritedText>().unwrap_or_default();

    inherited.base = inherited.base.overlay(self.base);
    inherited.link = inherited.link.overlay(self.link);

    let scope = Scope::current().create_child();

    scope.enter(|| {
      Context::provide(inherited);

      self.child.build()
    })
  }
}
