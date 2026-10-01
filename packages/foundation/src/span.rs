//! Inline runs, and the paragraph they flow in.

use std::rc::Rc;

use crate::{
  AnyKind,
  AnyView,
  Bubbling,
  ClickEvent,
  ClickHandler,
  Handler,
  Layout,
  LayoutBox,
  NodeId,
  PlainChild,
  Platform,
  RunDisplay,
  Scene,
  TypeMetrics,
  View,
  layout_box::verdict,
  realize::Source,
  run::Run,
};

/// What a span renders.
enum Content<P: Platform> {
  Text(Source<String>),
  Group(Vec<Span<P>>),
  View(AnyView<P>),
}

/// An inline run within a [`RichText`]: text, a group of spans styled as a unit, or a
/// view embedded among the text.
///
/// Spans flow together and wrap as one paragraph, so styling can vary within a line. A
/// property left unset is inherited through its [`typography`](crate::typography)
/// token. A [group](Self::group) redefines the typography tokens of the properties it
/// sets for the spans beneath it, its type metrics among them, on its own run: a
/// derived leaf is redefined with each value it computes, and a variant conditioned on
/// a state applies to the group's own run alone. A [view](Self::view) is laid out as an
/// inline-level box among the surrounding runs.
#[must_use]
pub struct Span<P: Platform> {
  content: Content<P>,
  run: Run<P>,
  click: Vec<Rc<ClickHandler>>,
}

impl<P: Platform> std::fmt::Debug for Span<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Span").finish_non_exhaustive()
  }
}

impl<P: Platform> Span<P> {
  /// A run reading `content`.
  pub fn new(content: impl Into<String>) -> Self {
    Self::with_content(Content::Text(Source::Fixed(content.into())))
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self::with_content(Content::Text(Source::derive(content)))
  }

  /// A group of spans styled as a unit.
  pub fn group(spans: impl IntoIterator<Item = Self>) -> Self {
    Self::with_content(Content::Group(spans.into_iter().collect()))
  }

  /// A view embedded inline among the surrounding text.
  pub fn view(view: impl View<P>) -> Self {
    Self::with_content(Content::View(view.into_any()))
  }

  #[inline(always)]
  fn with_content(content: Content<P>) -> Self {
    Self {
      content,
      run: Run::new(),
      click: Vec::new(),
    }
  }

  crate::run_surface! { run, P }

  /// Runs `action` on a click on the span; its verdict says whether the click carries
  /// on to the ancestors.
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self
      .click
      .push(Rc::new(move |_: &ClickEvent| verdict(action())));

    self
  }
}

impl<P: Platform> AnyKind for Span<P> {}

impl<P: Platform> View<P> for Span<P>
where
  P::StyleExt: TypeMetrics,
{
  fn build(self) -> NodeId {
    let node = match self.content {
      Content::View(_) => self.run.build(RunDisplay::InlineBox),
      Content::Text(_) => self.run.build(RunDisplay::Inline),
      Content::Group(_) => self.run.build_group(RunDisplay::Inline),
    };

    Scene::<P>::with(|scene| {
      for handler in self.click {
        scene.handle(node, Handler::Click(handler));
      }
    });

    match self.content {
      Content::Text(content) => {
        content.bind(node, |node, text| P::set_text(node, text));
      }
      Content::Group(spans) => {
        for span in spans {
          let child = span.build();

          Scene::<P>::with(|scene| scene.attach(node, child, None));
        }
      }
      Content::View(view) => {
        let child = view.build();

        Scene::<P>::with(|scene| scene.attach(node, child, None));
      }
    }

    node
  }
}

/// A paragraph of [`Span`]s, flowing together and wrapping as one.
///
/// Carries no properties of its own; decorating it means wrapping it in a
/// [`Container`](crate::Container).
#[derive(Debug)]
#[must_use]
pub struct RichText<P: Platform> {
  spans: Vec<Span<P>>,
}

impl<P: Platform> Default for RichText<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> RichText<P> {
  /// An empty paragraph.
  #[inline(always)]
  pub const fn new() -> Self {
    Self { spans: Vec::new() }
  }

  /// Appends spans to the paragraph.
  pub fn set_spans(mut self, spans: impl IntoIterator<Item = Span<P>>) -> Self {
    self.spans.extend(spans);

    self
  }
}

impl<P: Platform> AnyKind for RichText<P> {}

impl<P: Platform> View<P> for RichText<P>
where
  P::StyleExt: TypeMetrics,
{
  fn build(self) -> NodeId {
    let node = LayoutBox::<P, PlainChild>::new(Layout::Flow).build();

    for span in self.spans {
      let child = span.build();

      Scene::<P>::with(|scene| scene.attach(node, child, None));
    }

    node
  }
}

/// Implements a facade's newtype `$name` over the foundation's [`Span`].
///
/// The newtype holds the view on the platform `$platform` in its field; the macro
/// implements the constructors, the builder surface, the click handler and the traits a
/// span meets.
#[macro_export]
macro_rules! impl_span {
  ($name:ident, $platform:ty) => {
    impl ::core::fmt::Debug for $name {
      fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&self.0, formatter)
      }
    }
    impl $name {
      /// A run reading `content`.
      #[inline(always)]
      pub fn new(content: impl Into<String>) -> Self {
        Self($crate::Span::new(content))
      }

      /// A run whose text is recomputed whenever a signal `content` reads changes.
      #[inline(always)]
      pub fn derive(content: impl Fn() -> String + 'static) -> Self {
        Self($crate::Span::derive(content))
      }

      /// A group of spans styled as a unit.
      pub fn group(spans: impl IntoIterator<Item = Self>) -> Self {
        Self($crate::Span::group(spans.into_iter().map(|span| span.0)))
      }

      /// A view embedded inline among the surrounding text.
      #[inline(always)]
      pub fn view(view: impl $crate::View<$platform>) -> Self {
        Self($crate::Span::view(view))
      }

      $crate::run_surface! { 0, $platform }

      /// Runs `action` on a click on the span; its verdict says whether the click
      /// carries on to the ancestors.
      #[inline(always)]
      pub fn handle_click<R: Into<$crate::Bubbling>>(
        self,
        action: impl Fn() -> R + 'static,
      ) -> Self {
        Self(self.0.handle_click(action))
      }
    }

    impl $crate::AnyKind for $name {}

    impl $crate::View<$platform> for $name {
      #[inline(always)]
      fn build(self) -> $crate::NodeId {
        $crate::View::build(self.0)
      }
    }
  };
}

/// Implements a facade's newtype `$name` over the foundation's [`RichText`].
///
/// The newtype holds the view on the platform `$platform` in its field, its spans the
/// facade's newtype `$span`; the macro implements the constructor, the spans and the
/// traits a paragraph meets.
#[macro_export]
macro_rules! impl_rich_text {
  ($name:ident, $span:ident, $platform:ty) => {
    impl ::core::fmt::Debug for $name {
      fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&self.0, formatter)
      }
    }
    impl ::core::default::Default for $name {
      #[inline(always)]
      fn default() -> Self {
        Self::new()
      }
    }
    impl $name {
      /// An empty paragraph.
      #[inline(always)]
      pub const fn new() -> Self {
        Self($crate::RichText::new())
      }

      /// Appends spans to the paragraph.
      pub fn set_spans(self, spans: impl IntoIterator<Item = $span>) -> Self {
        Self(self.0.set_spans(spans.into_iter().map(|span| span.0)))
      }
    }

    impl $crate::AnyKind for $name {}

    impl $crate::View<$platform> for $name {
      #[inline(always)]
      fn build(self) -> $crate::NodeId {
        $crate::View::build(self.0)
      }
    }
  };
}
