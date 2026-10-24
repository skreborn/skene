//! Inline runs, and the paragraph they flow in.

use std::rc::Rc;

use crate::{
  AnyKind,
  AnyView,
  Bubbling,
  ClickEvent,
  ClickHandler,
  DeclaredRunStyle,
  Handler,
  HasRun,
  Layout,
  LayoutBox,
  NodeId,
  PlainChild,
  Prevent,
  Realization,
  RunBase,
  RunPlacement,
  Scene,
  TypeMetrics,
  View,
  Wraps,
  contents,
  realize::Source,
  run::Run,
};

/// What a span renders.
enum Content<P: Realization> {
  Text(Source<Rc<str>>),
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
pub struct Span<P: Realization> {
  content: Content<P>,
  run: Run<P>,
  click: Vec<Rc<ClickHandler>>,
}

impl<P: Realization> std::fmt::Debug for Span<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Span").finish_non_exhaustive()
  }
}

impl<P: Realization> Span<P> {
  /// A run reading `content`.
  pub fn new(content: impl Into<String>) -> Self {
    Self::with_content(Content::Text(Source::Fixed(Rc::from(content.into()))))
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self::with_content(Content::Text(Source::derive(move || Rc::from(content()))))
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

  /// Runs `action` on a click on the span; its verdict says whether the click carries
  /// on to the ancestors.
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.click.push(Rc::new(move |_: &ClickEvent| {
      let bubbling: Bubbling = action().into();

      Prevent::from(bubbling)
    }));

    self
  }
}

impl<P: Realization> AnyKind for Span<P> {}

impl<P: Realization> View<P> for Span<P>
where
  P::StyleExt: TypeMetrics,
{
  fn build(self) -> NodeId {
    let node = match self.content {
      Content::View(_) => self.run.build(RunPlacement::Embedded),
      Content::Text(_) => self.run.build(RunPlacement::Inline),
      Content::Group(_) => self.run.build_group(RunPlacement::Inline),
    };

    Scene::<P>::with(|scene| {
      for handler in self.click {
        scene.handle(node, Handler::Click(handler));
      }
    });

    match self.content {
      Content::Text(content) => {
        content.bind(node, |node, text| contents::TEXT.write::<P>(node, text));
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
pub struct RichText<P: Realization> {
  spans: Vec<Span<P>>,
}

impl<P: Realization> Default for RichText<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> RichText<P> {
  /// An empty paragraph.
  #[inline(always)]
  pub const fn new() -> Self {
    Self { spans: Vec::new() }
  }

  /// Appends spans to the paragraph.
  pub fn add_spans(mut self, spans: impl IntoIterator<Item = Span<P>>) -> Self {
    self.spans.extend(spans);

    self
  }
}

impl<P: Realization> AnyKind for RichText<P> {}

impl<P: Realization> View<P> for RichText<P>
where
  P::StyleExt: TypeMetrics,
{
  fn build(self) -> NodeId {
    let node = LayoutBox::<P, PlainChild>::new(Layout::Document).build();

    for span in self.spans {
      let child = span.build();

      Scene::<P>::with(|scene| scene.attach(node, child, None));
    }

    node
  }
}

impl<P: Realization> HasRun<P> for Span<P> {
  #[inline(always)]
  fn run_style_mut(&mut self) -> &mut DeclaredRunStyle<P> {
    self.run.style_mut()
  }

  #[inline(always)]
  fn map_run(self, map: impl FnOnce(Run<P>) -> Run<P>) -> Self {
    Self {
      content: self.content,
      run: map(self.run),
      click: self.click,
    }
  }
}

impl<P: Realization> RunBase<P> for Span<P> {}

/// What a facade's [`Span`] carries beyond its run: its constructors and its click handler.
pub trait SpanBase<P: Realization>: Wraps<Inner = Span<P>> {
  /// A run reading `content`.
  #[must_use]
  fn new(content: impl Into<String>) -> Self {
    Self::wrap(Span::new(content))
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[must_use]
  fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self::wrap(Span::derive(content))
  }

  /// A group of spans styled as a unit.
  #[must_use]
  fn group(spans: impl IntoIterator<Item = Self>) -> Self {
    Self::wrap(Span::group(spans.into_iter().map(Wraps::into_inner)))
  }

  /// A view embedded inline among the surrounding text.
  #[must_use]
  fn view(view: impl View<P>) -> Self {
    Self::wrap(Span::view(view))
  }

  /// Runs `action` on a click on the span; its verdict says whether the click carries
  /// on to the ancestors.
  #[must_use]
  fn handle_click<R: Into<Bubbling>>(self, action: impl Fn() -> R + 'static) -> Self {
    Self::wrap(self.into_inner().handle_click(action))
  }
}

/// What a facade's [`RichText`] carries: its constructor and its spans.
pub trait RichTextBase<P: Realization>: Wraps<Inner = RichText<P>> {
  /// An empty paragraph.
  #[must_use]
  fn new() -> Self {
    Self::wrap(RichText::new())
  }

  /// Appends spans to the paragraph, each a facade's newtype over a [`Span`].
  #[must_use]
  fn add_spans<S: Wraps<Inner = Span<P>>>(self, spans: impl IntoIterator<Item = S>) -> Self {
    Self::wrap(
      self
        .into_inner()
        .add_spans(spans.into_iter().map(Wraps::into_inner)),
    )
  }
}
