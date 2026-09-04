//! The setter pair every property has.

/// Generates a `set_`/`derive_` pair per property: the setter takes anything
/// convertible into the property's [`Value`](skene_core::next::Value) under the
/// driver's conditions, the deriver a closure over signals, and both write the field
/// named on the style `$access` returns.
macro_rules! properties {
  ($access:ident; $($set:ident / $derive:ident ($param:ident) => $field:ident: $type:ty, $doc:literal;)+) => {$(
    #[doc = concat!("Sets ", $doc, ".")]
    #[inline(always)]
    pub fn $set(
      mut self,
      $param: impl Into<
        ::skene_core::next::Value<$type, ::skene_core::next::Condition<D::Scalar>>,
      >,
    ) -> Self {
      self.$access().$field = Some($param.into());

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads.")]
    #[inline(always)]
    pub fn $derive(mut self, $param: impl Fn() -> $type + 'static) -> Self {
      self.$access().$field = Some(::skene_core::next::Value::new(
        ::skene_core::next::Leaf::Derived(::std::rc::Rc::new($param)),
      ));

      self
    }
  )+};
}

/// Generates the decoration and sizing surface the one decorated box and every control
/// share, over the style `$access` returns.
macro_rules! decoration {
  ($access:ident) => {
    $crate::macros::properties! { $access;
      set_width / derive_width (width) => width: ::skene_core::Dimension<D::Scalar>,
        "the width";
      set_height / derive_height (height) => height: ::skene_core::Dimension<D::Scalar>,
        "the height";
      set_min_width / derive_min_width (min_width) => min_width: ::skene_core::Dimension<D::Scalar>,
        "the minimum width";
      set_min_height / derive_min_height (min_height)
        => min_height: ::skene_core::Dimension<D::Scalar>,
        "the minimum height";
      set_max_width / derive_max_width (max_width) => max_width: ::skene_core::Dimension<D::Scalar>,
        "the maximum width";
      set_max_height / derive_max_height (max_height)
        => max_height: ::skene_core::Dimension<D::Scalar>,
        "the maximum height";
      set_aspect_ratio / derive_aspect_ratio (aspect_ratio) => aspect_ratio: f32,
        "the width-to-height ratio the unsized axis follows";
      set_padding / derive_padding (padding) => padding: ::skene_core::Inset<D::Scalar>,
        "the padding on each edge";
      set_margin / derive_margin (margin)
        => margin: ::skene_core::Inset<::skene_core::Dimension<D::Scalar>>,
        "the margin on each edge";
      set_offset / derive_offset (offset)
        => offset: ::skene_core::Inset<::skene_core::Dimension<D::Scalar>>,
        "the shift from each edge, keeping the box in flow";
      set_color / derive_color (color) => color: ::skene_core::Color,
        "the background color";
      set_border / derive_border (border) => border: ::skene_core::next::Border<D::Scalar>,
        "the border";
      set_cursor / derive_cursor (cursor) => cursor: ::skene_core::MouseCursor,
        "the mouse cursor shown over the box";
      set_overflow / derive_overflow (overflow) => overflow: ::skene_core::Overflow,
        "how content exceeding the box's bounds is treated; `Scroll` and `Auto` make it scrollable";
    }
  };
}

pub(crate) use decoration;
pub(crate) use properties;
