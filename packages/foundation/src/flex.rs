//! The flex layouts: a row and a column.

use crate::{
  AnyKind,
  Children,
  FlexChild,
  FlexDirection,
  HasChildren,
  Layout,
  LayoutBox,
  NodeId,
  Platform,
  Seal,
  View,
};

/// Generates a flex layout's builder surface on a type holding one in `$field`.
///
/// The surface is the declared style and the properties a flex container honors, on the
/// platform `$platform`. The foundation's [`Row`] and [`Column`] expand it over their
/// box, and a facade's newtype over the layout it wraps.
#[macro_export]
macro_rules! flex_surface {
  ($field:tt, $platform:ty) => {
    /// The declared style, for a facade's medium-specific setters to write into.
    #[inline(always)]
    pub const fn style_mut(&mut self) -> &mut $crate::DeclaredBoxStyle<$platform> {
      self.$field.style_mut()
    }

    $crate::properties! { style_mut, $platform;
      set_padding / derive_padding (padding) => padding: $crate::Inset<<$platform as $crate::Platform>::Scalar>,
        "the inset between the box and the children it arranges";
      set_spacing / derive_spacing (spacing) => spacing: $crate::Spacing<<$platform as $crate::Platform>::Scalar>,
        "the spacing between children per axis; a plain length spaces both";
      set_arrangement / derive_arrangement (arrangement) => arrangement: $crate::Arrangement,
        "the distribution of children along the main axis";
      set_item_alignment / derive_item_alignment (alignment) => item_alignment: $crate::ItemAlignment,
        "the alignment of each child on the cross axis";
      set_line_arrangement / derive_line_arrangement (arrangement) => line_arrangement: $crate::Arrangement,
        "the distribution of wrapped lines along the cross axis";
      set_wrap / derive_wrap (wrap) => wrap: $crate::Wrap,
        "whether and how children wrap onto further lines";
    }
  };
}

/// Generates a flex layout view over [`LayoutBox`], exposing the properties a flex
/// container honors.
macro_rules! flex_layout {
  ($name:ident, $doc:literal, $direction:expr, $reversed:expr) => {
    #[doc = concat!("Children laid out ", $doc, ".")]
    ///
    /// Carries layout properties and nothing else; decorating it means wrapping it in
    /// a [`Container`](crate::Container). Accepts any view or a
    /// [`Flexible`](crate::Flexible) wrapper as a child.
    #[derive(Debug)]
    #[must_use]
    pub struct $name<P: Platform> {
      inner: LayoutBox<P, FlexChild>,
    }

    impl<P: Platform> Default for $name<P> {
      #[inline(always)]
      fn default() -> Self {
        Self::new()
      }
    }

    impl<P: Platform> $name<P> {
      #[doc = concat!("An empty layout running ", $doc, ".")]
      #[inline(always)]
      pub fn new() -> Self {
        Self {
          inner: LayoutBox::new(Layout::Flex {
            direction: $direction,
          }),
        }
      }

      #[doc = concat!("An empty layout running ", $doc, ", reversed.")]
      #[inline(always)]
      pub fn reversed() -> Self {
        Self {
          inner: LayoutBox::new(Layout::Flex {
            direction: $reversed,
          }),
        }
      }

      crate::flex_surface! { inner, P }
    }

    impl<P: Platform> HasChildren<P> for $name<P> {
      type Kind = FlexChild;

      #[inline(always)]
      fn children_mut(&mut self, seal: Seal) -> &mut Children<P, FlexChild> {
        self.inner.children_mut(seal)
      }
    }

    impl<P: Platform> AnyKind for $name<P> {}

    impl<P: Platform> View<P> for $name<P> {
      #[inline(always)]
      fn build(self) -> NodeId {
        self.inner.build()
      }
    }
  };
}

flex_layout!(
  Row,
  "left to right",
  FlexDirection::Row,
  FlexDirection::RowReverse
);
flex_layout!(
  Column,
  "top to bottom",
  FlexDirection::Column,
  FlexDirection::ColumnReverse
);
