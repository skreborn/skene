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
        $crate::Value<$type, $crate::Condition<<$platform as $crate::Realization>::Scalar>>,
      >,
    ) -> Self {
      self.$access().$($field).+ = Some($param.into());

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, or a token or an expression where the property takes one.")]
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

    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, or a token or an expression where the property takes one.")]
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
        $crate::Value<$crate::Alignment, $crate::Condition<<$platform as $crate::Realization>::Scalar>>,
      >,
    ) -> Self {
      let (horizontal, vertical) = alignment.into().split_axes();
      let style = self.$access();

      style.item_horizontal_alignment = Some(horizontal);
      style.item_alignment = Some(vertical);

      self
    }

    #[doc = concat!("Derives ", $doc, " from the signals `alignment` reads.")]
    pub fn $derive<R: Into<$crate::StaticLeaf<$crate::Alignment>>>(
      self,
      alignment: impl Fn() -> R + 'static,
    ) -> Self {
      self.$set($crate::Value::<
        $crate::Alignment,
        $crate::Condition<<$platform as $crate::Realization>::Scalar>,
      >::new($crate::Leaf::derive(alignment)))
    }
  };
}

/// Generates builder methods forwarding to the same-named methods of the field `$field`,
/// or of the field `$field` of the field `$outer` (`members.inner`), each rebuilding the
/// field and returning `self`.
///
/// A method's generic parameters sit in brackets after its name:
/// `fn handle_click[R: Into<Bubbling>](action: impl Fn() -> R + 'static);`.
#[macro_export]
macro_rules! forward {
  ($outer:ident . $field:ident; $(
    $(#[$meta:meta])*
    fn $name:ident $([$($generics:tt)*])? ($($argument:ident: $type:ty),* $(,)?);
  )+) => {$(
    $(#[$meta])*
    #[inline(always)]
    pub fn $name $(<$($generics)*>)? (mut self, $($argument: $type),*) -> Self {
      self.$outer.$field = self.$outer.$field.$name($($argument),*);

      self
    }
  )+};
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
