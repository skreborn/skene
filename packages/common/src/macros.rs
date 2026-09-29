//! The setter pair every property has, and the setter surfaces built from it; a view
//! outside this crate generates its setters with the same macros.

/// Generates a `set_`/`derive_` pair per property.
///
/// The setter takes anything convertible into the property's
/// [`Value`](skene_core::Value) under the platform's conditions, the deriver a closure
/// over signals, and both write the field named on the style `$access` returns.
#[macro_export]
macro_rules! properties {
  ($access:ident; $($set:ident / $derive:ident ($param:ident) => $field:ident: $type:ty, $doc:literal;)+) => {$(
    #[doc = concat!("Sets ", $doc, ".")]
    #[inline(always)]
    pub fn $set(
      mut self,
      $param: impl Into<
        $crate::__core::Value<$type, $crate::__core::Condition<P::Scalar>>,
      >,
    ) -> Self {
      self.$access().$field = Some($param.into());

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, a token or an expression.")]
    #[inline(always)]
    pub fn $derive<R: Into<$crate::__core::StaticLeaf<$type>>>(
      mut self,
      $param: impl Fn() -> R + 'static,
    ) -> Self {
      self.$access().$field = Some($crate::__core::Value::new(
        $crate::__core::Leaf::derive($param),
      ));

      self
    }
  )+};
}

/// Generates the decoration and sizing surface the one decorated box and every control
/// share, over the style `$access` returns.
#[macro_export]
macro_rules! decoration {
  ($access:ident) => {
    $crate::properties! { $access;
      set_width / derive_width (width) => width: $crate::__core::Dimension<P::Scalar>,
        "the width";
      set_height / derive_height (height) => height: $crate::__core::Dimension<P::Scalar>,
        "the height";
      set_min_width / derive_min_width (min_width) => min_width: $crate::__core::Dimension<P::Scalar>,
        "the minimum width";
      set_min_height / derive_min_height (min_height)
        => min_height: $crate::__core::Dimension<P::Scalar>,
        "the minimum height";
      set_max_width / derive_max_width (max_width) => max_width: $crate::__core::Dimension<P::Scalar>,
        "the maximum width";
      set_max_height / derive_max_height (max_height)
        => max_height: $crate::__core::Dimension<P::Scalar>,
        "the maximum height";
      set_aspect_ratio / derive_aspect_ratio (aspect_ratio) => aspect_ratio: f32,
        "the width-to-height ratio the unsized axis follows";
      set_padding / derive_padding (padding) => padding: $crate::__core::Inset<P::Scalar>,
        "the padding on each edge";
      set_margin / derive_margin (margin)
        => margin: $crate::__core::Inset<$crate::__core::Dimension<P::Scalar>>,
        "the margin on each edge";
      set_offset / derive_offset (offset)
        => offset: $crate::__core::Inset<$crate::__core::Dimension<P::Scalar>>,
        "the shift from each edge, keeping the box in flow";
      set_color / derive_color (color) => color: $crate::__core::Color,
        "the background color";
      set_border / derive_border (border) => border: $crate::__core::Border<P::Scalar>,
        "the border";
      set_cursor / derive_cursor (cursor) => cursor: $crate::__core::MouseCursor,
        "the mouse cursor shown over the box";
      set_overflow / derive_overflow (overflow) => overflow: $crate::__core::Overflow,
        "how content exceeding the box's bounds is treated; `Scroll` and `Auto` make it scrollable";
    }

    /// Sets the corner radius, in the medium's unit, where the medium rounds corners; a
    /// medium without radii accepts and ignores it.
    #[inline(always)]
    pub fn set_corner_radius(
      mut self,
      radius: impl Into<$crate::__core::Value<f32, $crate::__core::Condition<P::Scalar>>>,
    ) -> Self
    where
      P::StyleExt: $crate::CornerRadius,
    {
      <P::StyleExt as $crate::CornerRadius>::set_corner_radius::<
        $crate::__core::Declared<$crate::__core::Condition<P::Scalar>>,
      >(&mut self.$access().ext, Some(radius.into()));

      self
    }

    /// Derives the corner radius from the signals `radius` reads, as a value, a token
    /// or an expression, where the medium rounds corners; a medium without radii
    /// accepts and ignores it.
    #[inline(always)]
    pub fn derive_corner_radius<R: Into<$crate::__core::StaticLeaf<f32>>>(
      mut self,
      radius: impl Fn() -> R + 'static,
    ) -> Self
    where
      P::StyleExt: $crate::CornerRadius,
    {
      <P::StyleExt as $crate::CornerRadius>::set_corner_radius::<
        $crate::__core::Declared<$crate::__core::Condition<P::Scalar>>,
      >(
        &mut self.$access().ext,
        Some($crate::__core::Value::new($crate::__core::Leaf::derive(radius))),
      );

      self
    }
  };
}

/// Generates the type-metric setters every run shares, over the style `$access`
/// returns, each writing through the extension's [`TypeMetrics`](crate::TypeMetrics).
#[macro_export]
macro_rules! type_metrics {
  ($access:ident; $($set:ident / $derive:ident ($param:ident) => $write:ident, $doc:literal;)+) => {$(
    #[doc = concat!("Sets ", $doc, ", where the medium sizes type; a medium without type metrics accepts and ignores it.")]
    #[inline(always)]
    pub fn $set(
      mut self,
      $param: impl Into<$crate::__core::Value<f32, $crate::__core::Condition<P::Scalar>>>,
    ) -> Self
    where
      P::StyleExt: $crate::TypeMetrics,
    {
      <P::StyleExt as $crate::TypeMetrics>::$write::<
        $crate::__core::Declared<$crate::__core::Condition<P::Scalar>>,
      >(&mut self.$access().ext, Some($param.into()));

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads, as a value, a token or an expression, where the medium sizes type; a medium without type metrics accepts and ignores it.")]
    #[inline(always)]
    pub fn $derive<R: Into<$crate::__core::StaticLeaf<f32>>>(
      mut self,
      $param: impl Fn() -> R + 'static,
    ) -> Self
    where
      P::StyleExt: $crate::TypeMetrics,
    {
      <P::StyleExt as $crate::TypeMetrics>::$write::<
        $crate::__core::Declared<$crate::__core::Condition<P::Scalar>>,
      >(
        &mut self.$access().ext,
        Some($crate::__core::Value::new($crate::__core::Leaf::derive($param))),
      );

      self
    }
  )+};
}

/// The four type-metric setter pairs a run exposes.
#[macro_export]
macro_rules! run_type_metrics {
  ($access:ident) => {
    $crate::type_metrics! { $access;
      set_font_size / derive_font_size (size) => set_font_size, "the font size";
      set_font_weight / derive_font_weight (weight) => set_font_weight, "the font weight";
      set_line_height / derive_line_height (height) => set_line_height,
        "the line height, as a multiple of the font size";
      set_letter_spacing / derive_letter_spacing (spacing) => set_letter_spacing,
        "the extra space between letters";
    }
  };
}
