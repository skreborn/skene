//! A box a paint callback draws into.

use std::rc::Rc;

use crate::{
  AnyKind,
  AnyView,
  Canvas,
  Enhance,
  Layout,
  LayoutBox,
  NodeId,
  Painter,
  Painting,
  Role,
  Size,
  View,
  realize::ChildBuilder,
};

/// Generates a [`Painted`]'s style surface on a type holding a box in `$field`.
///
/// The surface is the box's declared style, its sizing and its handle, on the platform
/// `$platform`. The foundation's painted box expands it over its box, and a facade's
/// newtype over the painted box it wraps.
#[macro_export]
macro_rules! painted_surface {
  ($field:tt, $platform:ty) => {
    /// The declared style, for a facade's medium-specific setters to write into.
    #[inline(always)]
    pub const fn style_mut(&mut self) -> &mut $crate::DeclaredBoxStyle<$platform> {
      self.$field.style_mut()
    }

    $crate::properties! { style_mut, $platform;
      set_width / derive_width (width) => width: $crate::Dimension<<$platform as $crate::Platform>::Scalar>,
        "the width";
      set_height / derive_height (height) => height: $crate::Dimension<<$platform as $crate::Platform>::Scalar>,
        "the height";
      set_min_width / derive_min_width (min_width) => min_width: $crate::Dimension<<$platform as $crate::Platform>::Scalar>,
        "the minimum width";
      set_min_height / derive_min_height (min_height)
        => min_height: $crate::Dimension<<$platform as $crate::Platform>::Scalar>,
        "the minimum height";
      set_max_width / derive_max_width (max_width) => max_width: $crate::Dimension<<$platform as $crate::Platform>::Scalar>,
        "the maximum width";
      set_max_height / derive_max_height (max_height)
        => max_height: $crate::Dimension<<$platform as $crate::Platform>::Scalar>,
        "the maximum height";
      set_aspect_ratio / derive_aspect_ratio (aspect_ratio)
        => aspect_ratio: $crate::AspectRatio,
        "the width-to-height ratio the unsized axis follows";
    }

    $crate::forward! { $field;
      /// Binds `handle` to the box once built: its measured size is reported through the
      /// handle, and an overlay may be placed against it.
      fn bind_handle(handle: $crate::NodeHandle<$platform>);
    }
  };
}

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
  pub fn new(painter: impl Fn(&mut Canvas, Size<f32>) + 'static) -> Self {
    Self {
      inner: LayoutBox::new(Layout::Flow),
      painter: Rc::new(painter),
      fallback: None,
    }
  }

  crate::painted_surface! { inner, P }

  /// Sets what the picture shows, for assistive technology: the box is then a picture
  /// named by `description`; without one it is decoration.
  pub fn set_description(mut self, description: impl Into<String>) -> Self {
    self.inner = self.inner.set_role(Role::Image).set_label(description);

    self
  }

  /// Sets the view built where nothing is drawn as pixels; nothing by default.
  pub fn set_fallback<V: View<P>>(mut self, fallback: impl FnOnce() -> V + 'static) -> Self {
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

/// Implements a facade's newtype `$name` over the foundation's [`Painted`].
///
/// The newtype holds the painted box on the platform `$platform` in its field; the
/// macro implements the constructor, the style surface, the description, the fallback
/// and the traits a painted box meets.
#[macro_export]
macro_rules! impl_painted {
  ($name:ident, $platform:ty) => {
    impl ::core::fmt::Debug for $name {
      fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&self.0, formatter)
      }
    }

    impl $name {
      /// A box painted by `painter`.
      #[inline(always)]
      pub fn new(painter: impl Fn(&mut $crate::Canvas, $crate::Size<f32>) + 'static) -> Self {
        Self($crate::Painted::new(painter))
      }

      $crate::painted_surface! { 0, $platform }

      $crate::forward! { 0;
        /// Sets what the picture shows, for assistive technology: the box is then a
        /// picture named by `description`; without one it is decoration.
        fn set_description(description: impl Into<String>);
        /// Sets the view built where nothing is drawn as pixels; nothing by default.
        fn set_fallback[V: $crate::View<$platform>](fallback: impl FnOnce() -> V + 'static);
      }
    }

    impl $crate::AnyKind for $name {}

    impl $crate::View<$platform> for $name {
      #[inline(always)]
      fn build(self) -> $crate::NodeId {
        $crate::View::build(self.0)
      }
    }
  };
}
