//! A text run on its own line.

use skene_core::{
  Animation,
  AnyKind,
  Color,
  DeclaredRunStyle,
  Driver,
  LinkPolicy,
  NodeId,
  RunDisplay,
  RunKeyframe,
  StyleExtension,
  TextDecoration,
  TextDecorationStyle,
  Transition,
  View,
};

use crate::{properties, realize::Source, run::Run, run_type_metrics};

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token; the medium's own text properties come through the driver's style extension.
#[must_use]
pub struct Text<D: Driver> {
  content: Source<String>,
  run: Run<D>,
}

impl<D: Driver> Text<D> {
  /// A run reading `content`.
  #[inline(always)]
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      content: Source::Fixed(content.into()),
      run: Run::new(),
    }
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[inline(always)]
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      content: Source::derive(content),
      run: Run::new(),
    }
  }

  /// The declared style, for a facade's medium-specific setters to write into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredRunStyle<D> {
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

  /// Sets whether the reader can select and copy the text; off by default.
  #[inline(always)]
  pub fn set_selectable(mut self, selectable: bool) -> Self {
    self.run = self.run.set_selectable(selectable);

    self
  }

  /// Makes the text a link to `destination`, inked in the link color unless styled and
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

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<D::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.run = self.run.set_transition(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<RunKeyframe<D>>) -> Self {
    self.run = self.run.set_animation(animation);

    self
  }
}

impl<D: Driver> AnyKind for Text<D> {}

impl<D: Driver> View<D> for Text<D> {
  fn build(self) -> NodeId {
    let node = self.run.build(RunDisplay::Block, false);

    self
      .content
      .bind::<D>(node, |sink, text| D::default().set_text(sink, text));

    node
  }
}
