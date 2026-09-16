//! A box a paint callback draws into.

use std::rc::Rc;

use skene_core::{AnyKind, AnyView, IntoView, Layout, NodeId, Size, View};

use crate::{
  Canvas,
  Enhance,
  LayoutBox,
  Painter,
  Painting,
  SizeHandle,
  properties,
  realize::{ChildBuilder, with_sink},
};

/// A box drawn by a paint callback through the driver's renderer: sized like any box,
/// painted at its laid-out size now and whenever that size changes.
///
/// The portable low-level path for pixels; it exists only where the driver
/// implements [`Painting`]. With a [`fallback`](Self::set_fallback) it is an
/// [`Enhance`] on [`Painting::available`]: the fallback stands where nothing is drawn
/// as pixels, a server render and a terminal without a graphics protocol included,
/// and the painted box takes its place once something is.
#[must_use]
pub struct Painted<D: Painting> {
  inner: LayoutBox<D>,
  painter: Rc<Painter>,
  fallback: Option<ChildBuilder<D>>,
}

impl<D: Painting> Painted<D> {
  /// A box painted by `painter`.
  #[inline(always)]
  pub fn new(painter: impl Fn(&mut Canvas, Size<f32>) + 'static) -> Self {
    Self {
      inner: LayoutBox::new(Layout::Flow),
      painter: Rc::new(painter),
      fallback: None,
    }
  }

  fn style_mut(&mut self) -> &mut skene_core::DeclaredBoxStyle<D> {
    self.inner.style_mut()
  }

  properties! { style_mut;
    set_width / derive_width (width) => width: skene_core::Dimension<D::Scalar>,
      "the width";
    set_height / derive_height (height) => height: skene_core::Dimension<D::Scalar>,
      "the height";
    set_min_width / derive_min_width (min_width) => min_width: skene_core::Dimension<D::Scalar>,
      "the minimum width";
    set_min_height / derive_min_height (min_height)
      => min_height: skene_core::Dimension<D::Scalar>,
      "the minimum height";
    set_max_width / derive_max_width (max_width) => max_width: skene_core::Dimension<D::Scalar>,
      "the maximum width";
    set_max_height / derive_max_height (max_height)
      => max_height: skene_core::Dimension<D::Scalar>,
      "the maximum height";
    set_aspect_ratio / derive_aspect_ratio (aspect_ratio) => aspect_ratio: f32,
      "the width-to-height ratio the unsized axis follows";
  }

  /// Binds `size` to the box once built, so its measured size is reported through it.
  #[inline(always)]
  pub fn bind_size(mut self, size: &SizeHandle<D>) -> Self {
    self.inner = self.inner.bind_size(size);

    self
  }

  /// Sets the view built where nothing is drawn as pixels; nothing by default.
  #[inline(always)]
  pub fn set_fallback<V: IntoView<D>>(mut self, fallback: impl FnOnce() -> V + 'static) -> Self {
    self.fallback = Some(ChildBuilder::new(fallback));

    self
  }
}

impl<D: Painting> AnyKind for Painted<D> {}

impl<D: Painting> View<D> for Painted<D> {
  fn build(self) -> NodeId {
    let Self {
      inner,
      painter,
      fallback,
    } = self;
    let paint = move || {
      let node = inner.build();

      with_sink::<D, _>(node, |sink| {
        let painter = Rc::clone(&painter);

        D::default().paint(sink, Box::new(move |canvas, size| painter(canvas, size)));
      });

      node
    };
    let Some(fallback) = fallback
    else {
      return paint();
    };

    Enhance::<D>::new(D::default().available())
      .set_fallback(move || AnyView::<D>::new(move || fallback.build()))
      .set_child_builder(move || AnyView::<D>::new(paint))
      .build()
  }
}
