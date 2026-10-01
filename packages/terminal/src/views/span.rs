//! Inline runs, and the paragraph they flow in.

use skene_foundation::{AnyKind, Bubbling, NodeId, View};

use crate::ActivePlatform;

/// An inline run within a [`RichText`]: text, a group of spans styled as a unit, or a
/// view embedded among the text.
///
/// Spans flow together and wrap as one paragraph, so styling can vary within a line. A
/// property left unset is inherited through its [`typography`](crate::typography)
/// token. A [group](Self::group) redefines the typography tokens of the properties it
/// sets for the spans beneath it, its type metrics among them, on its own run: a derived
/// leaf is redefined with each value it computes, and a variant conditioned on a state
/// applies to the group's own run alone. A [view](Self::view) is laid out as an
/// inline-level box among the surrounding runs.
#[derive(Debug)]
#[must_use]
pub struct Span(skene_foundation::Span<ActivePlatform>);

impl Span {
  /// A run reading `content`.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self(skene_foundation::Span::new(content))
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self(skene_foundation::Span::derive(content))
  }

  /// A group of spans styled as a unit.
  pub fn group(spans: impl IntoIterator<Item = Self>) -> Self {
    Self(skene_foundation::Span::group(
      spans.into_iter().map(|span| span.0),
    ))
  }

  /// A view embedded inline among the surrounding text.
  #[inline(always)]
  pub fn view(view: impl View<ActivePlatform>) -> Self {
    Self(skene_foundation::Span::view(view))
  }

  skene_foundation::run_surface! { 0, ActivePlatform }

  skene_foundation::properties! { style_mut, ActivePlatform;
    set_bold / derive_bold (bold) => ext.bold: bool, "whether the text is bold";
  }

  /// Runs `action` on a click on the span; its verdict says whether the click carries
  /// on to the ancestors.
  #[inline(always)]
  pub fn handle_click<R: Into<Bubbling>>(self, action: impl Fn() -> R + 'static) -> Self {
    Self(self.0.handle_click(action))
  }
}

impl AnyKind for Span {}

impl View<ActivePlatform> for Span {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}

/// A paragraph of [`Span`]s, flowing together and wrapping as one.
///
/// Carries no properties of its own; decorating it means wrapping it in a
/// [`Container`](crate::Container).
#[derive(Debug, Default)]
#[must_use]
pub struct RichText(skene_foundation::RichText<ActivePlatform>);

impl RichText {
  /// An empty paragraph.
  #[inline(always)]
  pub const fn new() -> Self {
    Self(skene_foundation::RichText::new())
  }

  /// Appends spans to the paragraph.
  pub fn set_spans(self, spans: impl IntoIterator<Item = Span>) -> Self {
    Self(self.0.set_spans(spans.into_iter().map(|span| span.0)))
  }
}

impl AnyKind for RichText {}

impl View<ActivePlatform> for RichText {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.0.build()
  }
}
