//! The properties only one medium has, hung off the shared styles.

use std::fmt::Debug;

use super::{BoxFieldVisitor, PropertyName, RunFieldVisitor, RunValue, Scalar, Shape};

/// A medium's own properties, added to [`BoxStyle`](super::BoxStyle) and
/// [`RunStyle`](super::RunStyle) beside the shared ones.
///
/// A facade implements it once on a zero-sized marker: the field blocks per shape, the
/// names of its properties, the erased value an update of one carries, and the field
/// enumeration the shared machinery walks. `()` is the extension with nothing in it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a style extension",
  label = "a facade's medium-specific properties are a zero-sized type implementing `StyleExtension`; `()` adds none"
)]
pub trait StyleExtension: Copy + Default + PartialEq + Debug + 'static {
  /// The box properties, one field per property in shape `S`.
  type BoxFields<S: Shape>: Default + Clone + PartialEq + Debug;
  /// The run properties, one field per property in shape `S`.
  type RunFields<S: Shape>: Default + Clone + PartialEq + Debug;
  /// The names of the properties, each naming one property, of the box or of the run.
  type Property: PropertyName;
  /// An erased value of one of the properties, for a derived slot's update.
  type Value: Clone + PartialEq + Debug + 'static;

  /// Visits every box field, in declaration order.
  fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V);

  /// Visits every run field, in declaration order.
  fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V);

  /// The run value a numeric token or expression yields for this medium's run
  /// properties, if any; a medium with no numeric run property yields none.
  #[inline(always)]
  #[must_use]
  fn run_value_from_number(number: f32) -> Option<RunValue<Self>> {
    let _ = number;

    None
  }

  /// The value `amount` of the way from `from` to `to`, for a platform running motion on
  /// its own clock; `None` for a discrete pair, as every pair of an extension whose
  /// values have no in-between is.
  #[inline(always)]
  #[must_use]
  fn interpolate_value(
    _from: &Self::Value,
    _to: &Self::Value,
    _amount: f32,
  ) -> Option<Self::Value> {
    None
  }
}

/// Declares a style extension's properties as types.
///
/// It makes the module `$module`, holding one field type per property, a
/// [`BoxField`](crate::BoxField) or a [`RunField`](crate::RunField) over the extension's
/// field blocks, and `visit_box` and `visit_run`, visiting them in declaration order, for
/// the extension's [`for_each_box_field`](StyleExtension::for_each_box_field) and
/// [`for_each_run_field`](StyleExtension::for_each_run_field).
///
/// Each property names its field in the extension's block, its value type, which the
/// module resolves as the invoking module does, and its variant of the extension's
/// property enum, which the field type is named for.
///
/// ```ignore
/// skene_foundation::extension_fields! {
///   pub mod acme_fields for AcmeExt, AcmeProperty;
///   box: tint: Color => Tint;
///   run: tracking: Length => Tracking,
/// }
/// ```
#[macro_export]
macro_rules! extension_fields {
  (
    $(#[$meta:meta])*
    $vis:vis mod $module:ident for $ext:ident, $property:ident;
    box: $($box_field:ident: $box_value:ty => $box_property:ident),* $(,)?;
    run: $($run_field:ident: $run_value:ty => $run_property:ident),* $(,)?
  ) => {
    $(#[$meta])*
    $vis mod $module {
      #[allow(unused_imports, reason = "the property's value types resolve as the invoking module does")]
      use super::*;

      $(
        #[doc = concat!("The `", stringify!($box_field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $box_property;

        impl<N: $crate::Scalar> $crate::BoxField<N, super::$ext> for $box_property {
          type Value = $box_value;

          const KEY: $crate::PropertyKey<super::$property> =
            $crate::PropertyKey::Extension(super::$property::$box_property);

          fn get<S: $crate::Shape>(style: &$crate::BoxStyle<N, S, super::$ext>) -> &S::Of<$box_value> {
            &style.ext.$box_field
          }

          fn get_mut<S: $crate::Shape>(
            style: &mut $crate::BoxStyle<N, S, super::$ext>,
          ) -> &mut S::Of<$box_value> {
            &mut style.ext.$box_field
          }
        }
      )*

      $(
        #[doc = concat!("The `", stringify!($run_field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $run_property;

        impl $crate::RunField<super::$ext> for $run_property {
          type Value = $run_value;

          const KEY: $crate::PropertyKey<super::$property> =
            $crate::PropertyKey::Extension(super::$property::$run_property);

          fn get<S: $crate::Shape>(style: &$crate::RunStyle<S, super::$ext>) -> &S::Of<$run_value> {
            &style.ext.$run_field
          }

          fn get_mut<S: $crate::Shape>(
            style: &mut $crate::RunStyle<S, super::$ext>,
          ) -> &mut S::Of<$run_value> {
            &mut style.ext.$run_field
          }
        }
      )*

      /// Visits every box property, in declaration order.
      pub fn visit_box<N: $crate::Scalar, V: $crate::BoxFieldVisitor<N, super::$ext>>(
        visitor: &mut V,
      ) {
        $(visitor.field::<$box_property>();)*
        let _ = visitor;
      }

      /// Visits every run property, in declaration order.
      pub fn visit_run<V: $crate::RunFieldVisitor<super::$ext>>(visitor: &mut V) {
        $(visitor.field::<$run_property>();)*
        let _ = visitor;
      }
    }
  };
}

/// The property names of an extension with no properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoProperty {}

impl PropertyName for NoProperty {
  #[inline(always)]
  fn name(&self) -> &'static str {
    match *self {}
  }
}

/// The erased value of an extension with no properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoValue {}

impl StyleExtension for () {
  type BoxFields<S: Shape> = ();
  type Property = NoProperty;
  type RunFields<S: Shape> = ();
  type Value = NoValue;

  fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(_: &mut V) {}

  fn for_each_run_field<V: RunFieldVisitor<Self>>(_: &mut V) {}
}
