//! A box a paint callback draws into.

use std::rc::Rc;

use skene_core::{AnyKind, Layout, NodeId, Size, View};
use skene_views::{LayoutBox, SizeHandle, properties, realize::with_sink};

use crate::{Canvas, Painter, Painting};

/// A box drawn by a paint callback through the driver's renderer: sized like any box,
/// painted at its laid-out size now and whenever that size changes.
///
/// The portable low-level path for pixel media; it exists only where the driver
/// implements [`Painting`].
#[must_use]
pub struct Painted<D: Painting> {
  inner: LayoutBox<D>,
  painter: Rc<Painter>,
}

impl<D: Painting> Painted<D> {
  /// A box painted by `painter`.
  #[inline(always)]
  pub fn new(painter: impl Fn(&mut Canvas, Size<f32>) + 'static) -> Self {
    Self {
      inner: LayoutBox::new(Layout::Flow),
      painter: Rc::new(painter),
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
}

impl<D: Painting> AnyKind for Painted<D> {}

impl<D: Painting> View<D> for Painted<D> {
  fn build(self) -> NodeId {
    let Self { inner, painter } = self;
    let node = inner.build();

    with_sink::<D, _>(node, |sink| {
      let painter = Rc::clone(&painter);

      D::default().paint(sink, Box::new(move |canvas, size| painter(canvas, size)));
    });

    node
  }
}
