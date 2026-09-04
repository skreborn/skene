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

pub(crate) use properties;
