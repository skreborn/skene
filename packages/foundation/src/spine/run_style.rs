//! A text run's style: every property both facades share, in one shape.

use std::{fmt::Debug, rc::Rc};

use super::{
  Declared,
  DerivedSlot,
  Fingerprint,
  Keyframe,
  Plain,
  Property,
  PropertyKey,
  Shape,
  Slot,
  Static,
  StyleExtension,
};
use crate::spine::{Animation, Color, Length, TextDecoration, TextDecorationStyle, Transition};

/// A text run's style: one field per shared property in shape `S`, and the medium's own
/// properties in `ext`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct RunStyle<S: Shape, X: StyleExtension = ()> {
  /// The text color.
  pub color: S::Of<Color>,
  /// The text color while selected.
  pub selection_color: S::Of<Color>,
  /// The background behind selected text.
  pub selection_background: S::Of<Color>,
  /// The line the text is decorated with; a link is underlined unless set.
  pub decoration: S::Of<TextDecoration>,
  /// How the decoration line is drawn.
  pub decoration_style: S::Of<TextDecorationStyle>,
  /// The decoration line's color; the text color unless set.
  pub decoration_color: S::Of<Color>,
  /// The medium's own properties.
  pub ext: X::RunFields<S>,
}

/// A run property's value, erased over the property: what a derived slot computes and
/// an update carries.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum RunValue<X: StyleExtension = ()> {
  /// A color.
  Color(Color),
  /// A length in the medium's unit, its fractions kept.
  Length(Length),
  /// A decoration line.
  Decoration(TextDecoration),
  /// A decoration line's style.
  DecorationStyle(TextDecorationStyle),
  /// A value of one of the medium's own properties.
  Extension(X::Value),
}

/// A type a run property holds, erasable into a [`RunValue`].
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a run property type under this extension",
  label = "a run property holds a shared value type, or one of the medium's own"
)]
pub trait IntoRunValue<X: StyleExtension>:
  Clone + PartialEq + Debug + Fingerprint + 'static
{
  /// This value, erased.
  fn into_run_value(self) -> RunValue<X>;

  /// This type's reading of an erased value; `None` when it holds another type.
  fn from_run_value(value: RunValue<X>) -> Option<Self>;
}

/// The shared run value types, each erasing into its variant.
macro_rules! run_values {
  ($($type:ty => $variant:ident),+ $(,)?) => {$(
    impl<X: StyleExtension> IntoRunValue<X> for $type {
      #[inline(always)]
      fn into_run_value(self) -> RunValue<X> {
        RunValue::$variant(self)
      }

      #[inline(always)]
      fn from_run_value(value: RunValue<X>) -> Option<Self> {
        match value {
          RunValue::$variant(value) => Some(value),
          _ => None,
        }
      }
    }
  )+};
}

run_values!(
  Color => Color,
  Length => Length,
  TextDecoration => Decoration,
  TextDecorationStyle => DecorationStyle,
);

/// One property of a [`RunStyle`], addressed by type, in whatever shape the style is.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a run property",
  label = "a run property is one of the types in `run_fields`, or one the style extension enumerates"
)]
pub trait RunField<X: StyleExtension>: 'static {
  /// The type the property holds.
  type Value: IntoRunValue<X>;

  /// The property's name.
  const KEY: PropertyKey<X::Property>;

  /// The field in `style`.
  fn get<S: Shape>(style: &RunStyle<S, X>) -> &S::Of<Self::Value>;

  /// The field in `style`, mutably.
  fn get_mut<S: Shape>(style: &mut RunStyle<S, X>) -> &mut S::Of<Self::Value>;
}

/// Visits a [`RunStyle`]'s properties by type.
pub trait RunFieldVisitor<X: StyleExtension> {
  /// Visits the property `F`.
  fn field<F: RunField<X>>(&mut self);
}

/// Visits a [`RunStyle`]'s fields by value, each with its name.
pub trait RunVisitor<S: Shape, X: StyleExtension> {
  /// Visits the field of property `key`, holding a `T`.
  fn value<T: IntoRunValue<X>>(&mut self, key: PropertyKey<X::Property>, value: &S::Of<T>);
}

/// A derived run leaf a split lifted out.
pub type RunSlot<X = ()> = DerivedSlot<PropertyKey<<X as StyleExtension>::Property>, RunValue<X>>;

/// The shared run properties as types, and the enumeration walking them.
macro_rules! run_fields {
  ($($field:ident: $value:ty => $property:ident),+ $(,)?) => {
    /// The shared run properties, addressed by type.
    pub mod run_fields {
      use super::{Property, PropertyKey, RunField, RunStyle, Shape, StyleExtension};

      $(
        #[doc = concat!("The `", stringify!($field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $property;

        impl<X: StyleExtension> RunField<X> for $property {
          type Value = $value;

          const KEY: PropertyKey<X::Property> = PropertyKey::Shared(Property::$property);

          fn get<S: Shape>(style: &RunStyle<S, X>) -> &S::Of<$value> {
            &style.$field
          }

          fn get_mut<S: Shape>(style: &mut RunStyle<S, X>) -> &mut S::Of<$value> {
            &mut style.$field
          }
        }
      )+
    }

    impl<S: Shape, X: StyleExtension> RunStyle<S, X> {
      /// Visits every property, the shared ones in declaration order and then the
      /// extension's.
      pub fn for_each_field<V: RunFieldVisitor<X>>(visitor: &mut V) {
        $(visitor.field::<run_fields::$property>();)+
        X::for_each_run_field(visitor);
      }
    }
  };
}

run_fields!(
  color: crate::spine::Color => TextColor,
  selection_color: crate::spine::Color => SelectionColor,
  selection_background: crate::spine::Color => SelectionBackground,
  decoration: crate::spine::TextDecoration => Decoration,
  decoration_style: crate::spine::TextDecorationStyle => DecorationStyle,
  decoration_color: crate::spine::Color => DecorationColor,
);

impl<S: Shape, X: StyleExtension> RunStyle<S, X> {
  /// Visits every field with its name.
  pub fn visit<V: RunVisitor<S, X>>(&self, visitor: &mut V) {
    Self::for_each_field(&mut Visit {
      style: self,
      visitor,
    });
  }
}

struct Visit<'a, S: Shape, X: StyleExtension, V> {
  style: &'a RunStyle<S, X>,
  visitor: &'a mut V,
}

impl<S: Shape, X: StyleExtension, V: RunVisitor<S, X>> RunFieldVisitor<X> for Visit<'_, S, X, V> {
  fn field<F: RunField<X>>(&mut self) {
    self.visitor.value::<F::Value>(F::KEY, F::get(self.style));
  }
}

impl<X: StyleExtension> RunStyle<Plain, X> {
  /// Lays `over` on this style: every property `over` sets replaces this one's.
  #[inline(always)]
  pub fn overlay(&mut self, over: &Self) {
    Self::for_each_field(&mut Overlay { base: self, over });
  }
}

struct Overlay<'a, X: StyleExtension> {
  base: &'a mut RunStyle<Plain, X>,
  over: &'a RunStyle<Plain, X>,
}

impl<X: StyleExtension> RunFieldVisitor<X> for Overlay<'_, X> {
  fn field<F: RunField<X>>(&mut self) {
    if let Some(value) = F::get(self.over) {
      *F::get_mut(self.base) = Some(value.clone());
    }
  }
}

impl<C: Clone + PartialEq + Debug + 'static, X: StyleExtension> RunStyle<Declared<C>, X> {
  /// The static style a platform receives, and the derived leaves lifted out of it: each
  /// slot's index is its position in the list, and its property's field holds
  /// [`KeptLeaf::Slot`](super::KeptLeaf::Slot) where the leaf was.
  pub fn split(mut self) -> (RunStyle<Static<C>, X>, Vec<RunSlot<X>>) {
    let mut target = RunStyle::default();
    let mut slots = Vec::new();

    Self::for_each_field(&mut Split {
      source: &mut self,
      target: &mut target,
      slots: &mut slots,
    });

    (target, slots)
  }
}

struct Split<'a, C: Clone + PartialEq + Debug + 'static, X: StyleExtension> {
  source: &'a mut RunStyle<Declared<C>, X>,
  target: &'a mut RunStyle<Static<C>, X>,
  slots: &'a mut Vec<RunSlot<X>>,
}

impl<C: Clone + PartialEq + Debug + 'static, X: StyleExtension> RunFieldVisitor<X>
  for Split<'_, C, X>
{
  fn field<F: RunField<X>>(&mut self) {
    let Some(value) = F::get_mut(self.source).take()
    else {
      return;
    };

    let slots = &mut *self.slots;
    let split = value.split_derived(|position, compute| {
      let slot =
        Slot(u16::try_from(slots.len()).expect("a node holds at most 65536 derived slots"));

      slots.push(DerivedSlot {
        slot,
        property: F::KEY,
        position,
        compute: Rc::new(move || compute().map(IntoRunValue::into_run_value)),
      });

      slot
    });

    *F::get_mut(self.target) = Some(split);
  }
}

/// A run's motion: the transitions between its values and the keyframe animations it
/// plays.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct RunDynamics<X: StyleExtension = ()> {
  /// The property transitions.
  pub transitions: Vec<Transition<X::Property>>,
  /// The keyframe animations, each keyframe a style of static leaves.
  pub animations: Vec<Animation<RunStyle<Keyframe, X>>>,
}

impl<X: StyleExtension> Default for RunDynamics<X> {
  #[inline(always)]
  fn default() -> Self {
    Self {
      transitions: Vec::new(),
      animations: Vec::new(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{RunStyle, RunValue};
  use crate::spine::{
    Color,
    Declared,
    KeptLeaf,
    Plain,
    Property,
    PropertyKey,
    Slot,
    State,
    StaticLeaf,
    TokenName,
    TokenRef,
  };

  /// Splitting a run keeps its token and lifts its derived leaf into a slot.
  #[test]
  fn split_yields_the_static_part_and_the_slot_list() {
    let declared = RunStyle::<Declared<State>> {
      color: Some(TokenRef::<Color>::new("ink").derive_when(State::Hovered, || Color::TRANSPARENT)),
      ..Default::default()
    };

    let (fixed, slots) = declared.split();
    let color = fixed.color.unwrap();

    assert_eq!(
      color.base(),
      &KeptLeaf::Static(StaticLeaf::Token(TokenName::new("ink")))
    );
    assert_eq!(color.variants()[0].1, KeptLeaf::Slot(Slot(0)));
    assert_eq!(fixed.selection_color, None);
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].property, PropertyKey::Shared(Property::TextColor));
    assert_eq!(slots[0].position, 1);
    assert_eq!(
      (slots[0].compute)(),
      StaticLeaf::Fixed(RunValue::Color(Color::TRANSPARENT))
    );
  }

  /// Overlaying replaces exactly the properties the overlay sets.
  #[test]
  fn overlay_later_wins_per_set_field() {
    let mut base = RunStyle::<Plain>::default();
    let mut over = RunStyle::<Plain>::default();

    base.color = Some(Color::srgb(1., 0., 0.));
    base.selection_color = Some(Color::srgb(0., 1., 0.));
    over.color = Some(Color::srgb(0., 0., 1.));

    base.overlay(&over);

    assert_eq!(base.color, Some(Color::srgb(0., 0., 1.)));
    assert_eq!(base.selection_color, Some(Color::srgb(0., 1., 0.)));
    assert_eq!(base.selection_background, None);
  }
}
