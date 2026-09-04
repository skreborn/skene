//! Inline runs, and the paragraph they flow in.

use skene_core::{
  Color,
  Layout,
  LinkPolicy,
  RunDisplay,
  next::{AnyKind, AnyView, DeclaredRunStyle, Driver, IntoView, NodeId, PlainChild, Scene, View},
};

use crate::{
  LayoutBox,
  macros::{properties, run_type_metrics},
  realize::Source,
  run::Run,
};

/// What a span renders.
enum Content<D: Driver> {
  Text(Source<String>),
  Group(Vec<Span<D>>),
  View(AnyView<D>),
}

/// An inline run within a [`RichText`]: text, a group of spans styled as a unit, or a
/// view embedded among the text.
///
/// Spans flow together and wrap as one paragraph, so styling can vary within a line. A
/// property left unset is inherited through its [`typography`](crate::typography)
/// token. A [group](Self::group) redefines the typography tokens of the properties it
/// sets for the spans beneath it, on its own run: a derived leaf is redefined with each
/// value it computes, and a variant conditioned on a state applies to the group's own
/// run alone. A [view](Self::view) is laid out as an inline-level box among the
/// surrounding runs.
#[must_use]
pub struct Span<D: Driver> {
  content: Content<D>,
  run: Run<D>,
}

impl<D: Driver> Span<D> {
  /// A run reading `content`.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self::with_content(Content::Text(Source::Fixed(content.into())))
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self::with_content(Content::Text(Source::derive(content)))
  }

  /// A group of spans styled as a unit.
  #[inline(always)]
  pub fn group(spans: impl IntoIterator<Item = Self>) -> Self {
    Self::with_content(Content::Group(spans.into_iter().collect()))
  }

  /// A view embedded inline among the surrounding text.
  #[inline(always)]
  pub fn view(view: impl IntoView<D>) -> Self {
    Self::with_content(Content::View(view.into_view()))
  }

  #[inline(always)]
  fn with_content(content: Content<D>) -> Self {
    Self {
      content,
      run: Run::new(),
    }
  }

  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredRunStyle<D> {
    self.run.style_mut()
  }

  properties! { style_mut;
    set_color / derive_color (color) => color: Color,
      "the text color";
    set_selection_color / derive_selection_color (color) => selection_color: Color,
      "the text color while selected";
    set_selection_background / derive_selection_background (color) => selection_background: Color,
      "the background behind selected text";
  }

  run_type_metrics! { style_mut }

  /// Sets whether the reader can select and copy the text; off by default.
  #[inline(always)]
  pub fn set_selectable(mut self, selectable: bool) -> Self {
    self.run = self.run.set_selectable(selectable);

    self
  }

  /// Makes the span a link to `destination`, inked in the link color unless styled and
  /// opening per its [`LinkPolicy`].
  #[inline(always)]
  pub fn set_link(mut self, destination: impl Into<String>) -> Self {
    self.run = self.run.set_link(destination);

    self
  }

  /// Sets how the link opens and what it discloses.
  #[inline(always)]
  pub fn set_link_policy(mut self, link_policy: LinkPolicy) -> Self {
    self.run = self.run.set_link_policy(link_policy);

    self
  }
}

impl<D: Driver> AnyKind for Span<D> {}

impl<D: Driver> View<D> for Span<D> {
  fn build(self) -> NodeId {
    let display = match self.content {
      Content::View(_) => RunDisplay::InlineBox,
      Content::Text(_) | Content::Group(_) => RunDisplay::Inline,
    };
    let group = matches!(self.content, Content::Group(_));
    let node = self.run.build(display, group);

    match self.content {
      Content::Text(content) => {
        content.bind::<D>(node, |sink, text| D::default().set_text(sink, text));
      }
      Content::Group(spans) => {
        for span in spans {
          let child = span.build();

          Scene::<D>::with(|scene| scene.attach(node, child, None));
        }
      }
      Content::View(view) => {
        let child = view.build();

        Scene::<D>::with(|scene| scene.attach(node, child, None));
      }
    }

    node
  }
}

/// A paragraph of [`Span`]s, flowing together and wrapping as one.
///
/// Carries no properties of its own; decorating it means wrapping it in a
/// [`Container`](crate::Container).
#[must_use]
pub struct RichText<D: Driver> {
  spans: Vec<Span<D>>,
}

impl<D: Driver> Default for RichText<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> RichText<D> {
  /// An empty paragraph.
  #[inline(always)]
  pub const fn new() -> Self {
    Self { spans: Vec::new() }
  }

  /// Appends spans to the paragraph.
  #[inline(always)]
  pub fn set_spans(mut self, spans: impl IntoIterator<Item = Span<D>>) -> Self {
    self.spans.extend(spans);

    self
  }
}

impl<D: Driver> AnyKind for RichText<D> {}

impl<D: Driver> View<D> for RichText<D> {
  fn build(self) -> NodeId {
    let node = LayoutBox::<D, PlainChild>::new(Layout::Flow).build();

    for span in self.spans {
      let child = span.build();

      Scene::<D>::with(|scene| scene.attach(node, child, None));
    }

    node
  }
}
