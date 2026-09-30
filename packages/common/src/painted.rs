//! A box a paint callback draws into.

use std::rc::Rc;

use skene_core::{AnyKind, AnyView, IntoView, Layout, NodeId, Role, Size, View};

use crate::{
  Canvas,
  Enhance,
  LayoutBox,
  NodeHandle,
  Painter,
  Painting,
  properties,
  realize::ChildBuilder,
};

/// A box drawn by a paint callback through the platform's renderer: sized like any box,
/// painted at its laid-out size now and whenever that size changes.
///
/// The portable low-level path for pixels; it exists only where the platform
/// implements [`Painting`]. With a [`fallback`](Self::set_fallback) it is an
/// [`Enhance`] on [`Painting::available`]: the fallback stands where nothing is drawn
/// as pixels, a server render and a terminal without a graphics protocol included,
/// and the painted box takes its place once something is.
#[must_use]
pub struct Painted<P: Painting> {
  inner: LayoutBox<P>,
  painter: Rc<Painter>,
  fallback: Option<ChildBuilder<P>>,
}

impl<P: Painting> std::fmt::Debug for Painted<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Painted").finish_non_exhaustive()
  }
}

impl<P: Painting> Painted<P> {
  /// A box painted by `painter`.
  #[inline(always)]
  pub fn new(painter: impl Fn(&mut Canvas, Size<f32>) + 'static) -> Self {
    Self {
      inner: LayoutBox::new(Layout::Flow),
      painter: Rc::new(painter),
      fallback: None,
    }
  }

  const fn style_mut(&mut self) -> &mut skene_core::DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }

  properties! { style_mut;
    set_width / derive_width (width) => width: skene_core::Dimension<P::Scalar>,
      "the width";
    set_height / derive_height (height) => height: skene_core::Dimension<P::Scalar>,
      "the height";
    set_min_width / derive_min_width (min_width) => min_width: skene_core::Dimension<P::Scalar>,
      "the minimum width";
    set_min_height / derive_min_height (min_height)
      => min_height: skene_core::Dimension<P::Scalar>,
      "the minimum height";
    set_max_width / derive_max_width (max_width) => max_width: skene_core::Dimension<P::Scalar>,
      "the maximum width";
    set_max_height / derive_max_height (max_height)
      => max_height: skene_core::Dimension<P::Scalar>,
      "the maximum height";
    set_aspect_ratio / derive_aspect_ratio (aspect_ratio)
      => aspect_ratio: skene_core::AspectRatio,
      "the width-to-height ratio the unsized axis follows";
  }

  crate::forward! { inner;
    /// Binds `handle` to the box once built: its measured size is reported through the
    /// handle, and an overlay may be placed against it.
    fn bind_handle(handle: NodeHandle<P>);
  }

  /// Sets what the picture shows, for assistive technology: the box is then a picture
  /// named by `description`; without one it is decoration.
  #[inline(always)]
  pub fn set_description(mut self, description: impl Into<String>) -> Self {
    self.inner = self.inner.set_role(Role::Image).set_label(description);

    self
  }

  /// Sets the view built where nothing is drawn as pixels; nothing by default.
  #[inline(always)]
  pub fn set_fallback<V: IntoView<P>>(mut self, fallback: impl FnOnce() -> V + 'static) -> Self {
    self.fallback = Some(ChildBuilder::new(fallback));

    self
  }
}

impl<P: Painting> AnyKind for Painted<P> {}

impl<P: Painting> View<P> for Painted<P> {
  fn build(self) -> NodeId {
    let Self {
      inner,
      painter,
      fallback,
    } = self;
    let paint = move || {
      let node = inner.build();

      {
        let painter = Rc::clone(&painter);

        P::paint(node, Box::new(move |canvas, size| painter(canvas, size)));
      }

      node
    };
    let Some(fallback) = fallback
    else {
      return paint();
    };

    Enhance::<P>::new(P::available())
      .set_fallback(move || AnyView::<P>::new(move || fallback.build()))
      .set_child_builder(move || AnyView::<P>::new(paint))
      .build()
  }
}
