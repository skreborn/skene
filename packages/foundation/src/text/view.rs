//! A text run on its own line.

use std::rc::Rc;

use crate::{
  AnyKind,
  DeclaredRunStyle,
  HasRun,
  NodeId,
  Realization,
  RunBase,
  RunPlacement,
  View,
  Wraps,
  contents,
  realize::Source,
  run::Run,
};

/// A run of text on its own line.
///
/// A property left unset is inherited through its [`typography`](crate::typography)
/// token; the medium's own text properties come through the platform's style extension.
#[must_use]
pub struct Text<P: Realization> {
  content: Source<Rc<str>>,
  run: Run<P>,
}

impl<P: Realization> std::fmt::Debug for Text<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Text").finish_non_exhaustive()
  }
}

impl<P: Realization> Text<P> {
  /// A run reading `content`.
  pub fn new(content: impl Into<String>) -> Self {
    Self {
      content: Source::Fixed(Rc::from(content.into())),
      run: Run::new(),
    }
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  pub fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self {
      content: Source::derive(move || Rc::from(content())),
      run: Run::new(),
    }
  }
}

impl<P: Realization> AnyKind for Text<P> {}

impl<P: Realization> View<P> for Text<P> {
  fn build(self) -> NodeId {
    let node = self.run.build(RunPlacement::Line);

    self
      .content
      .bind(node, |node, text| contents::TEXT.write::<P>(node, text));

    node
  }
}

impl<P: Realization> HasRun<P> for Text<P> {
  #[inline(always)]
  fn run_style_mut(&mut self) -> &mut DeclaredRunStyle<P> {
    self.run.style_mut()
  }

  #[inline(always)]
  fn map_run(self, map: impl FnOnce(Run<P>) -> Run<P>) -> Self {
    Self {
      content: self.content,
      run: map(self.run),
    }
  }
}

impl<P: Realization> RunBase<P> for Text<P> {}

/// The constructors of a facade's [`Text`].
pub trait TextBase<P: Realization>: Wraps<Inner = Text<P>> {
  /// A run reading `content`.
  #[must_use]
  fn new(content: impl Into<String>) -> Self {
    Self::wrap(Text::new(content))
  }

  /// A run whose text is recomputed whenever a signal `content` reads changes.
  #[must_use]
  fn derive(content: impl Fn() -> String + 'static) -> Self {
    Self::wrap(Text::derive(content))
  }
}
