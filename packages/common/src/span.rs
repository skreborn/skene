//! Inline runs, and the paragraph they flow in.

use std::rc::Rc;

use skene_core::{
  Animation,
  AnyKind,
  AnyView,
  Bubbling,
  ClickEvent,
  ClickHandler,
  Color,
  DeclaredRunStyle,
  Handler,
  IntoView,
  Layout,
  LinkPolicy,
  NodeId,
  PlainChild,
  Platform,
  RunDisplay,
  RunKeyframe,
  Scene,
  StyleExtension,
  TextDecoration,
  TextDecorationStyle,
  Transition,
  View,
};

use crate::{
  LayoutBox,
  TypeMetrics,
  layout_box::verdict,
  properties,
  realize::Source,
  run::Run,
  run_type_metrics,
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
  pub fn view(view: impl IntoView<P>) -> Self {
    Self::with_content(Content::View(view.into_view()))
  }

  #[inline(always)]
  fn with_content(content: Content<P>) -> Self {
    Self {
      content,
      run: Run::new(),
      click: Vec::new(),
    }
  }

  /// The declared style, for a facade's medium-specific setters to write into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredRunStyle<P> {
    self.run.style_mut()
  }

  properties! { style_mut;
    set_color / derive_color (color) => color: Color,
      "the text color";
    set_selection_color / derive_selection_color (color) => selection_color: Color,
      "the text color while selected";
    set_selection_background / derive_selection_background (color) => selection_background: Color,
      "the background behind selected text";
    set_decoration / derive_decoration (decoration) => decoration: TextDecoration,
      "the line the text is decorated with: beneath, above or through it; a link is underlined unless set";
    set_decoration_style / derive_decoration_style (style) => decoration_style: TextDecorationStyle,
      "how the decoration line is drawn";
    set_decoration_color / derive_decoration_color (color) => decoration_color: Color,
      "the decoration line's color; the text color unless set";
  }

  run_type_metrics! { style_mut }

  /// Runs `action` on a click on the span; its verdict says whether the click carries
  /// on to the ancestors.
  #[inline(always)]
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self
      .click
      .push(Rc::new(move |_: &ClickEvent| verdict(action())));

    self
  }

  /// Sets whether the reader can select and copy the text; off by default.
  #[inline(always)]
  pub fn set_selectable(mut self, selectable: bool) -> Self {
    self.run = self.run.set_selectable(selectable);

    self
  }

  /// Makes the span a link to `destination`, inked in the link color unless styled and
  /// opening per its [`LinkPolicy`]; a destination that would run script when followed,
  /// a `javascript:` URL, is never followed.
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

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.run = self.run.set_transition(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<RunKeyframe<P>>) -> Self {
    self.run = self.run.set_animation(animation);

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
        content.bind::<P>(node, |sink, text| P::set_text(sink, text));
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
  #[inline(always)]
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
