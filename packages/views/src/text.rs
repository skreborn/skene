//! A text run on its own line.

use skene_core::{
  Color,
  LinkPolicy,
  RunDisplay,
  next::{AnyKind, DeclaredRunStyle, Driver, NodeId, View},
};

use crate::{macros::properties, realize::Source, run::Run};

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
