//! The setter pair every property has, and the setter surfaces built from it; a view
//! outside this crate generates its setters with the same macros.

/// Generates a `set_`/`derive_` pair per property.
///
/// The setter takes anything convertible into the property's
/// [`Value`](skene_foundation::Value) under `$condition`, the facade's `Condition` on a
/// view written on a facade, the deriver a closure over signals, and both write the field
/// named on the style `$access` returns; a property's optional second literal is a
/// paragraph its setter's documentation adds.
///
/// A property line may start with `[cfg(<predicate>)]`, making it a platform's own
/// property of a shared view (`set_linux_*`, `set_apple_*`): its setter and deriver exist
/// in every build with the same signatures, write the field in a build where the predicate
/// holds and drop the value in every other, so the field, and the record entry it feeds,
/// exist under the same predicate alone. Their documentation names the predicate.
///
/// ```
/// use skene_foundation::{Color, Condition, Value};
///
/// /// A gauge's own style: its face, and the tint Linux draws its needle in.
/// pub struct GaugeStyle {
///   face: Option<Value<Color, Condition<f32>>>,
///   #[cfg(skene_platform = "linux")]
///   linux_tint: Option<Value<Color, Condition<f32>>>,
/// }
///
/// /// A gauge, a third party's view, its values under the conditions of a facade
/// /// measuring in `f32`.
/// pub struct Gauge {
///   style: GaugeStyle,
/// }
///
/// impl Gauge {
///   /// A gauge with nothing set.
///   pub const fn new() -> Self {
///     Self {
///       style: GaugeStyle {
///         face: None,
///         #[cfg(skene_platform = "linux")]
///         linux_tint: None,
///       },
///     }
///   }
///
///   const fn style_mut(&mut self) -> &mut GaugeStyle {
///     &mut self.style
///   }
///
///   skene_foundation::properties! { style_mut, Condition<f32>;
///     set_face / derive_face (color) => face: Color, "the face's color";
///     [cfg(skene_platform = "linux")]
///     set_linux_tint / derive_linux_tint (tint) => linux_tint: Color, "the needle's tint";
///   }
/// }
///
/// /// Compiles in every build; Linux's alone tints the needle.
/// pub fn gauge() -> Gauge {
///   Gauge::new()
///     .set_face(Color::oklch(0.98, 0.0, 0.0))
///     .set_linux_tint(Color::oklch(0.6, 0.2, 25.0))
/// }
/// ```
#[macro_export]
macro_rules! properties {
  (
    $access:ident, $condition:ty;
    $(
      $([cfg($predicate:meta)])?
      $set:ident / $derive:ident ($param:ident) => $($field:ident).+: $type:ty,
      $doc:literal $(, $detail:literal)?;
    )+
  ) => {$(
    $(#[cfg($predicate)])?
    #[doc = concat!("Sets ", $doc, ".")]
    $(#[doc = ""] #[doc = $detail])?
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
    pub fn $set(
      mut self,
      $param: impl Into<
        $crate::Value<$type, $condition>,
      >,
    ) -> Self {
      self.$access().$($field).+ = Some($param.into());

      self
    }

    $(#[cfg($predicate)])?
    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, or a token or an expression where the property takes one.")]
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
    pub fn $derive<R: Into<$crate::StaticLeaf<$type>>>(
      mut self,
      $param: impl Fn() -> R + 'static,
    ) -> Self {
      self.$access().$($field).+ = Some($crate::Value::new(
        $crate::Leaf::derive($param),
      ));

      self
    }

    #[cfg(not(all($($predicate)?)))]
    #[doc = concat!("Sets ", $doc, ".")]
    $(#[doc = ""] #[doc = $detail])?
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
    pub fn $set(
      self,
      $param: impl Into<
        $crate::Value<$type, $condition>,
      >,
    ) -> Self {
      drop($param);

      self
    }

    #[cfg(not(all($($predicate)?)))]
    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, or a token or an expression where the property takes one.")]
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
    pub fn $derive<R: Into<$crate::StaticLeaf<$type>>>(
      self,
      $param: impl Fn() -> R + 'static,
    ) -> Self {
      drop($param);

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
///
/// A property line may start with `[cfg(<predicate>)]`, making it a platform's own
/// property, as in [`properties!`](skene_foundation::properties): its pair exists in every
/// build, writes the field where the predicate holds and drops the value in every other.
#[macro_export]
macro_rules! trait_properties {
  (
    $access:ident, $condition:ty;
    $(
      $([cfg($predicate:meta)])?
      $set:ident / $derive:ident ($param:ident) => $($field:ident).+: $type:ty,
      $doc:literal $(, $detail:literal)?;
    )+
  ) => {$(
    $(#[cfg($predicate)])?
    #[doc = concat!("Sets ", $doc, ".")]
    $(#[doc = ""] #[doc = $detail])?
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
    #[must_use]
    fn $set(mut self, $param: impl Into<$crate::Value<$type, $condition>>) -> Self {
      self.$access().$($field).+ = Some($param.into());

      self
    }

    $(#[cfg($predicate)])?
    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, or a token or an expression where the property takes one.")]
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
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

    #[cfg(not(all($($predicate)?)))]
    #[doc = concat!("Sets ", $doc, ".")]
    $(#[doc = ""] #[doc = $detail])?
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
    #[must_use]
    fn $set(self, $param: impl Into<$crate::Value<$type, $condition>>) -> Self {
      drop($param);

      self
    }

    #[cfg(not(all($($predicate)?)))]
    #[doc = concat!("Derives ", $doc, " from the signals `", stringify!($param), "` reads: a value, or a token or an expression where the property takes one.")]
    $(
      #[doc = ""]
      #[doc = concat!("It acts in a build where `cfg(", stringify!($predicate), ")` holds, and in every other compiles and does nothing.")]
    )?
    #[must_use]
    fn $derive<R: Into<$crate::StaticLeaf<$type>>>(
      self,
      $param: impl Fn() -> R + 'static,
    ) -> Self {
      drop($param);

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
  ($access:ident, $condition:ty; $set:ident / $derive:ident, $doc:literal $(, $detail:literal)?) => {
    #[doc = concat!("Sets ", $doc, ", on both axes.")]
    $(#[doc = ""] #[doc = $detail])?
    pub fn $set(
      mut self,
      alignment: impl Into<
        $crate::Value<$crate::Alignment, $condition>,
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
      self.$set($crate::Value::<$crate::Alignment, $condition>::new($crate::Leaf::derive(alignment)))
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

#[cfg(test)]
mod tests {
  use crate::{
    BoxFieldVisitor,
    BoxStyle,
    BoxValue,
    BoxVisitor,
    Color,
    Condition,
    Declared,
    IntoBoxValue,
    Property,
    PropertyKey,
    PropertyName,
    RunFieldVisitor,
    Scalar,
    Shape,
    Static,
    StyleExtension,
  };

  /// A medium of the tests' own: a property the test build has, and one no build has.
  #[derive(Debug, Clone, Copy, Default, PartialEq)]
  struct Probe;

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  enum ProbeProperty {
    #[cfg(test)]
    Tint,
    #[cfg(any())]
    Gone,
  }

  impl PropertyName for ProbeProperty {
    fn name(&self) -> &'static str {
      match self {
        #[cfg(test)]
        Self::Tint => "tint",
        #[cfg(any())]
        Self::Gone => "gone",
      }
    }
  }

  #[derive(Debug, Clone, Default, PartialEq)]
  struct ProbeBox<S: Shape> {
    #[cfg(test)]
    tint: S::Of<u16>,
    #[cfg(any())]
    gone: S::Of<u16>,
  }

  impl<N: Scalar> IntoBoxValue<N, Probe> for u16 {
    fn into_box_value(self) -> BoxValue<N, Probe> {
      BoxValue::Extension(self)
    }

    fn from_box_value(value: BoxValue<N, Probe>) -> Option<Self> {
      match value {
        BoxValue::Extension(tint) => Some(tint),
        _ => None,
      }
    }
  }

  crate::extension_fields! {
    #[expect(unreachable_pub, reason = "the macro makes a facade's field types public")]
    mod probe_fields for Probe, ProbeProperty;
    box: [cfg(test)] tint: u16 => Tint, [cfg(any())] gone: u16 => Gone;
    run:
  }

  impl StyleExtension for Probe {
    type BoxFields<S: Shape> = ProbeBox<S>;
    type Property = ProbeProperty;
    type RunFields<S: Shape> = ();
    type Value = u16;

    fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V) {
      probe_fields::visit_box(visitor);
    }

    fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V) {
      probe_fields::visit_run(visitor);
    }
  }

  type Declaration = BoxStyle<f32, Declared<Condition<f32>>, Probe>;

  /// A view of the tests' own, with a shared property and the probe's two.
  #[derive(Default)]
  struct Gauge {
    style: Declaration,
  }

  trait GaugeBase: Sized {
    fn style_mut(&mut self) -> &mut Declaration;

    crate::trait_properties! { style_mut, Condition<f32>;
      set_background / derive_background (color) => background: Color, "the background";
      [cfg(test)]
      set_tint / derive_tint (tint) => ext.tint: u16, "the tint the test build has";
      [cfg(any())]
      set_gone / derive_gone (gone) => ext.gone: u16, "a property no build has";
    }
  }

  impl GaugeBase for Gauge {
    fn style_mut(&mut self) -> &mut Declaration {
      &mut self.style
    }
  }

  /// The properties a record built from a gauge carries, and its derived slots' count.
  fn record(gauge: Gauge) -> (Vec<PropertyKey<ProbeProperty>>, usize) {
    struct Entries(Vec<PropertyKey<ProbeProperty>>);

    impl BoxVisitor<f32, Static<Condition<f32>>, Probe> for Entries {
      fn value<T: IntoBoxValue<f32, Probe>>(
        &mut self,
        key: PropertyKey<ProbeProperty>,
        value: &<Static<Condition<f32>> as Shape>::Of<T>,
      ) {
        if value.is_some() {
          self.0.push(key);
        }
      }
    }

    let (style, slots) = gauge.style.split();
    let mut entries = Entries(Vec::new());

    style.visit(&mut entries);

    (entries.0, slots.len())
  }

  #[test]
  fn a_property_whose_predicate_holds_records_its_entry() {
    let tint = PropertyKey::Extension(ProbeProperty::Tint);

    assert_eq!(record(Gauge::default().set_tint(3_u16)), (vec![tint], 0));
    assert_eq!(
      record(Gauge::default().derive_tint(|| 3_u16)),
      (vec![tint], 1)
    );
  }

  #[test]
  fn a_property_whose_predicate_fails_compiles_and_records_nothing() {
    let background = PropertyKey::Shared(Property::Background);

    assert_eq!(record(Gauge::default().set_gone(3_u16)), (vec![], 0));
    assert_eq!(record(Gauge::default().derive_gone(|| 3_u16)), (vec![], 0));
    assert_eq!(
      record(
        Gauge::default()
          .set_gone(3_u16)
          .set_background(Color::oklch(0.5, 0.1, 30.0))
      ),
      (vec![background], 0),
    );
    assert_eq!(
      record(
        Gauge::default()
          .derive_gone(|| 3_u16)
          .derive_background(|| Color::oklch(0.5, 0.1, 30.0))
      ),
      (vec![background], 1),
    );
  }
}
