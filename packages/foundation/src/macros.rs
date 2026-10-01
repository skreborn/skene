//! The setter pair every property has, and the setter surfaces built from it; a view
//! outside this crate generates its setters with the same macros.

/// Generates a `set_`/`derive_` pair per property.
///
/// The setter takes anything convertible into the property's
/// [`Value`](skene_foundation::Value) under the platform's conditions, the deriver a closure
/// over signals, and both write the field named on the style `$access` returns.
#[macro_export]
macro_rules! properties {
  (
    $access:ident, $platform:ty;
    $(
      $set:ident / $derive:ident ($param:ident) => $($field:ident).+: $type:ty,
      $doc:literal $(, $detail:literal)?;
    )+
  ) => {$(
    #[doc = concat!("Sets ", $doc, ".")]
    $(#[doc = ""] #[doc = $detail])?
    pub fn $set(
      mut self,
      $param: impl Into<
        $crate::Value<$type, $crate::Condition<<$platform as $crate::Platform>::Scalar>>,
      >,
    ) -> Self {
      self.$access().$($field).+ = Some($param.into());

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, a token or an expression.")]
    pub fn $derive<R: Into<$crate::StaticLeaf<$type>>>(
      mut self,
      $param: impl Fn() -> R + 'static,
    ) -> Self {
      self.$access().$($field).+ = Some($crate::Value::new(
        $crate::Leaf::derive($param),
      ));

      self
    }
  )+};
}

/// Generates a provided `set_`/`derive_` pair per property on a trait, for a medium's
/// own properties over the style `$access` returns.
///
/// The setter takes anything convertible into the property's
/// [`Value`](skene_foundation::Value) under `$condition`, the deriver a closure over signals,
/// and both write the field at the path named; a property's optional second literal is
/// a paragraph its setter's documentation adds.
#[macro_export]
macro_rules! trait_properties {
  (
    $access:ident, $condition:ty;
    $(
      $set:ident / $derive:ident ($param:ident) => $($field:ident).+: $type:ty,
      $doc:literal $(, $detail:literal)?;
    )+
  ) => {$(
    #[doc = concat!("Sets ", $doc, ".")]
    $(#[doc = ""] #[doc = $detail])?
    #[must_use]
    fn $set(mut self, $param: impl Into<$crate::Value<$type, $condition>>) -> Self {
      self.$access().$($field).+ = Some($param.into());

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, a token or an expression.")]
    #[must_use]
    fn $derive<R: Into<$crate::StaticLeaf<$type>>>(
      mut self,
      $param: impl Fn() -> R + 'static,
    ) -> Self {
      self.$access().$($field).+ = Some($crate::Value::new(
        $crate::Leaf::derive($param),
      ));

      self
    }
  )+};
}

/// Generates a `set_`/`derive_` pair for a two-axis alignment, split onto the
/// horizontal and the vertical item alignment of the style `$access` returns.
///
/// The optional second literal is a paragraph the setter's documentation adds.
#[macro_export]
macro_rules! alignment_properties {
  ($access:ident, $platform:ty; $set:ident / $derive:ident, $doc:literal $(, $detail:literal)?) => {
    #[doc = concat!("Sets ", $doc, ", on both axes.")]
    $(#[doc = ""] #[doc = $detail])?
    pub fn $set(
      mut self,
      alignment: impl Into<
        $crate::Value<$crate::Alignment, $crate::Condition<<$platform as $crate::Platform>::Scalar>>,
      >,
    ) -> Self {
      let (horizontal, vertical) = alignment.into().split_axes();
      let style = self.$access();

      style.item_horizontal_alignment = Some(horizontal);
      style.item_alignment = Some(vertical);

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `alignment` reads: a value, a token or an expression.")]
    pub fn $derive<R: Into<$crate::StaticLeaf<$crate::Alignment>>>(
      self,
      alignment: impl Fn() -> R + 'static,
    ) -> Self {
      self.$set($crate::Value::<
        $crate::Alignment,
        $crate::Condition<<$platform as $crate::Platform>::Scalar>,
      >::new($crate::Leaf::derive(alignment)))
    }
  };
}

/// Generates the decoration and sizing surface the one decorated box and every control
/// share, over the style `$access` returns.
#[macro_export]
macro_rules! decoration {
  ($access:ident, $platform:ty) => {
    $crate::properties! { $access, $platform;
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
      set_padding / derive_padding (padding) => padding: $crate::Inset<<$platform as $crate::Platform>::Scalar>,
        "the padding on each edge";
      set_margin / derive_margin (margin)
        => margin: $crate::Inset<$crate::Dimension<<$platform as $crate::Platform>::Scalar>>,
        "the margin on each edge";
      set_offset / derive_offset (offset)
        => offset: $crate::Inset<$crate::Dimension<<$platform as $crate::Platform>::Scalar>>,
        "the shift from each edge, keeping the box in flow";
      set_color / derive_color (color) => color: $crate::Color,
        "the background color";
      set_border_width / derive_border_width (width)
        => border_width: $crate::Inset<<$platform as $crate::Platform>::Scalar>,
        "the border's width on each edge; zero draws none";
      set_border_color / derive_border_color (color) => border_color: $crate::Color,
        "the border's color";
      set_cursor / derive_cursor (cursor) => cursor: $crate::MouseCursor,
        "the mouse cursor shown over the box";
      set_overflow / derive_overflow (overflow)
        => overflow: $crate::Axes<$crate::Overflow>,
        "how content exceeding the box's bounds is treated on each axis; `Scroll` and `Auto` make an axis scrollable, and one overflow sets both";
      set_visibility / derive_visibility (visibility)
        => visibility: $crate::Visibility,
        "whether the box and its content are drawn; a hidden box keeps its place in the layout";
    }

    /// Sets the corner radius, a length in the medium's unit, where the medium rounds
    /// corners; a medium without radii accepts and ignores it.
    pub fn set_corner_radius(
      mut self,
      radius: impl Into<$crate::Value<$crate::Length, $crate::Condition<<$platform as $crate::Platform>::Scalar>>>,
    ) -> Self
    where
      <$platform as $crate::Platform>::StyleExt: $crate::CornerRadius,
    {
      <<$platform as $crate::Platform>::StyleExt as $crate::CornerRadius>::set_corner_radius::<
        $crate::Declared<$crate::Condition<<$platform as $crate::Platform>::Scalar>>,
      >(&mut self.$access().ext, Some(radius.into()));

      self
    }

    /// Derives the corner radius from the signals `radius` reads, as a value, a token
    /// or an expression, where the medium rounds corners; a medium without radii
    /// accepts and ignores it.
    pub fn derive_corner_radius<R: Into<$crate::StaticLeaf<$crate::Length>>>(
      mut self,
      radius: impl Fn() -> R + 'static,
    ) -> Self
    where
      <$platform as $crate::Platform>::StyleExt: $crate::CornerRadius,
    {
      <<$platform as $crate::Platform>::StyleExt as $crate::CornerRadius>::set_corner_radius::<
        $crate::Declared<$crate::Condition<<$platform as $crate::Platform>::Scalar>>,
      >(
        &mut self.$access().ext,
        Some($crate::Value::new($crate::Leaf::derive(radius))),
      );

      self
    }
  };
}

/// Generates builder methods forwarding to the same-named methods of the field
/// `$field`, each rebuilding the field and returning `self`.
///
/// A method's generic parameters sit in brackets after its name:
/// `fn handle_click[R: Into<Bubbling>](action: impl Fn() -> R + 'static);`.
#[macro_export]
macro_rules! forward {
  ($field:tt; $(
    $(#[$meta:meta])*
    fn $name:ident $([$($generics:tt)*])? ($($argument:ident: $type:ty),* $(,)?);
  )+) => {$(
    $(#[$meta])*
    #[inline(always)]
    pub fn $name $(<$($generics)*>)? (mut self, $($argument: $type),*) -> Self {
      self.$field = self.$field.$name($($argument),*);

      self
    }
  )+};
}

/// Generates a text view's builder surface on a type holding a run in `$field`.
///
/// The surface is the declared style, the color, selection and decoration setters, the
/// type metrics, selectability, the link and motion, on the platform `$platform`. The
/// foundation's text views expand it over their [`Run`](crate::Run), and a facade's
/// newtype over the view it wraps.
#[macro_export]
macro_rules! run_surface {
  ($field:tt, $platform:ty) => {
    /// The declared style, for a facade's medium-specific setters to write into.
    #[inline(always)]
    pub const fn style_mut(&mut self) -> &mut $crate::DeclaredRunStyle<$platform> {
      self.$field.style_mut()
    }

    $crate::properties! { style_mut, $platform;
      set_color / derive_color (color) => color: $crate::Color,
        "the text color";
      set_selection_color / derive_selection_color (color) => selection_color: $crate::Color,
        "the text color while selected";
      set_selection_background / derive_selection_background (color)
        => selection_background: $crate::Color,
        "the background behind selected text";
      set_decoration / derive_decoration (decoration) => decoration: $crate::TextDecoration,
        "the line the text is decorated with: beneath, above or through it; a link is underlined unless set";
      set_decoration_style / derive_decoration_style (style)
        => decoration_style: $crate::TextDecorationStyle,
        "how the decoration line is drawn";
      set_decoration_color / derive_decoration_color (color) => decoration_color: $crate::Color,
        "the decoration line's color; the text color unless set";
    }

    $crate::run_type_metrics! { style_mut, $platform }

    $crate::forward! { $field;
      /// Sets whether the reader can select and copy the text; off by default.
      fn set_selectable(selectable: bool);
      /// Makes the text a link to `destination`, inked in the link color unless styled
      /// and opening per its [`LinkPolicy`](skene_foundation::LinkPolicy); a destination that
      /// would run script when followed, a `javascript:` URL, is never followed.
      fn set_link(destination: impl Into<String>);
      /// Sets how the link opens and what it discloses.
      fn set_link_policy(link_policy: $crate::LinkPolicy);
      /// Adds a property transition, interpolating that property whenever its value
      /// changes.
      fn set_transition(
        transition: $crate::Transition<
          <<$platform as $crate::Platform>::StyleExt as $crate::StyleExtension>::Property,
        >,
      );
      /// Adds a keyframe animation.
      fn set_animation(animation: $crate::Animation<$crate::RunKeyframe<$platform>>);
    }
  };
}

/// Generates the type-metric setters every run shares, over the style `$access`
/// returns, each writing through the extension's [`TypeMetrics`](crate::TypeMetrics).
#[macro_export]
macro_rules! type_metrics {
  ($access:ident, $platform:ty; $($set:ident / $derive:ident ($param:ident: $value:ty) => $write:ident, $doc:literal;)+) => {$(
    #[doc = concat!("Sets ", $doc, ", where the medium sizes type; a medium without type metrics accepts and ignores it.")]
    pub fn $set(
      mut self,
      $param: impl Into<$crate::Value<$value, $crate::Condition<<$platform as $crate::Platform>::Scalar>>>,
    ) -> Self
    where
      <$platform as $crate::Platform>::StyleExt: $crate::TypeMetrics,
    {
      <<$platform as $crate::Platform>::StyleExt as $crate::TypeMetrics>::$write::<
        $crate::Declared<$crate::Condition<<$platform as $crate::Platform>::Scalar>>,
      >(&mut self.$access().ext, Some($param.into()));

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads, as a value, a token or an expression, where the medium sizes type; a medium without type metrics accepts and ignores it.")]
    pub fn $derive<R: Into<$crate::StaticLeaf<$value>>>(
      mut self,
      $param: impl Fn() -> R + 'static,
    ) -> Self
    where
      <$platform as $crate::Platform>::StyleExt: $crate::TypeMetrics,
    {
      <<$platform as $crate::Platform>::StyleExt as $crate::TypeMetrics>::$write::<
        $crate::Declared<$crate::Condition<<$platform as $crate::Platform>::Scalar>>,
      >(
        &mut self.$access().ext,
        Some($crate::Value::new($crate::Leaf::derive($param))),
      );

      self
    }
  )+};
}

/// The four type-metric setter pairs a run exposes.
#[macro_export]
macro_rules! run_type_metrics {
  ($access:ident, $platform:ty) => {
    $crate::type_metrics! { $access, $platform;
      set_font_size / derive_font_size (size: $crate::Length) => set_font_size,
        "the font size, a length in the medium's unit";
      set_font_weight / derive_font_weight (weight: f32) => set_font_weight,
        "the font weight";
      set_line_height / derive_line_height (height: f32) => set_line_height,
        "the line height, as a multiple of the font size";
      set_letter_spacing / derive_letter_spacing (spacing: $crate::Length)
        => set_letter_spacing, "the extra space between letters, a length in the medium's unit";
    }
  };
}
