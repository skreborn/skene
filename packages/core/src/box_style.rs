//! A box's style: every property both facades share, in one shape.

use std::{fmt::Debug, rc::Rc};

use super::{
  Border,
  Declared,
  DerivedSlot,
  Fingerprint,
  Keyframe,
  Plain,
  Property,
  PropertyKey,
  Scalar,
  Shape,
  Slot,
  Static,
  StyleExtension,
};
use crate::{
  Animation,
  Arrangement,
  Color,
  Dimension,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTrack,
  Inset,
  ItemAlignment,
  Line,
  MouseCursor,
  Overflow,
  Spacing,
  Transition,
  Visibility,
  Wrap,
};

/// A box's style: one field per shared property in shape `S`, lengths in `N`, and the
/// medium's own properties in `ext`.
///
/// Overlay, the static/derived split and every platform's rendering are written against
/// [`for_each_field`](Self::for_each_field); adding a property touches this struct, the
/// field list and each platform's mapping only.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct BoxStyle<N: Scalar, S: Shape, X: StyleExtension = ()> {
  /// The width.
  pub width: S::Of<Dimension<N>>,
  /// The height.
  pub height: S::Of<Dimension<N>>,
  /// The minimum width.
  pub min_width: S::Of<Dimension<N>>,
  /// The minimum height.
  pub min_height: S::Of<Dimension<N>>,
  /// The maximum width.
  pub max_width: S::Of<Dimension<N>>,
  /// The maximum height.
  pub max_height: S::Of<Dimension<N>>,
  /// The width-to-height ratio the unsized axis follows.
  pub aspect_ratio: S::Of<f32>,
  /// The margin on each edge.
  pub margin: S::Of<Inset<Dimension<N>>>,
  /// The padding on each edge.
  pub padding: S::Of<Inset<N>>,
  /// The shift from each edge, keeping the box in flow.
  pub offset: S::Of<Inset<Dimension<N>>>,
  /// The edges the box is pinned to, out of flow, against the nearest offset or
  /// pinned ancestor; wins when `offset` is also set.
  pub pinned: S::Of<Inset<Dimension<N>>>,
  /// The background color.
  pub color: S::Of<Color>,
  /// The border.
  pub border: S::Of<Border<N>>,
  /// The mouse cursor shown over the box.
  pub cursor: S::Of<MouseCursor>,
  /// The spacing between children, per axis.
  pub spacing: S::Of<Spacing<N>>,
  /// Whether and how a flex container wraps its children.
  pub wrap: S::Of<Wrap>,
  /// The box's flexibility as a flex child: its initial size and its grow and
  /// shrink weights.
  pub flex: S::Of<Flexibility<N>>,
  /// Distribution of children along the main (flex) or inline (grid) axis.
  pub arrangement: S::Of<Arrangement>,
  /// Alignment of children on the cross (flex) or block (grid) axis.
  pub item_alignment: S::Of<ItemAlignment>,
  /// Distribution of wrapped lines or grid tracks along the cross or block axis.
  pub line_arrangement: S::Of<Arrangement>,
  /// Alignment of grid children on the inline axis.
  pub item_horizontal_alignment: S::Of<ItemAlignment>,
  /// The box's own cross- or block-axis alignment, overriding its parent's
  /// `item_alignment`.
  pub self_alignment: S::Of<ItemAlignment>,
  /// The box's own inline-axis alignment in a grid, overriding its parent's
  /// `item_horizontal_alignment`.
  pub self_horizontal_alignment: S::Of<ItemAlignment>,
  /// The grid's explicit column tracks.
  pub grid_columns: S::Of<Vec<GridTrack<N>>>,
  /// The grid's explicit row tracks.
  pub grid_rows: S::Of<Vec<GridTrack<N>>>,
  /// The sizes given to implicitly created columns, cycled.
  pub grid_implicit_columns: S::Of<Vec<GridTrack<N>>>,
  /// The sizes given to implicitly created rows, cycled.
  pub grid_implicit_rows: S::Of<Vec<GridTrack<N>>>,
  /// The direction auto-placed grid children fill.
  pub grid_flow: S::Of<GridFlow>,
  /// The box's row placement as a grid child.
  pub grid_row_placement: S::Of<Line<GridPlacement>>,
  /// The box's column placement as a grid child.
  pub grid_column_placement: S::Of<Line<GridPlacement>>,
  /// How content exceeding the box's bounds is treated.
  pub overflow: S::Of<Overflow>,
  /// Whether the box and its content are drawn.
  pub visibility: S::Of<Visibility>,
  /// The medium's own properties.
  pub ext: X::BoxFields<S>,
}

/// A box property's value, erased over the property: what a derived slot computes and
/// an update carries.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum BoxValue<N: Scalar, X: StyleExtension = ()> {
  /// A length, fraction or auto.
  Dimension(Dimension<N>),
  /// A unitless number.
  Number(f32),
  /// A dimension per edge.
  DimensionInset(Inset<Dimension<N>>),
  /// A length per edge.
  Inset(Inset<N>),
  /// A color.
  Color(Color),
  /// A border.
  Border(Border<N>),
  /// A mouse cursor.
  Cursor(MouseCursor),
  /// A spacing per axis.
  Spacing(Spacing<N>),
  /// A wrapping mode.
  Wrap(Wrap),
  /// A flexibility.
  Flexibility(Flexibility<N>),
  /// An arrangement.
  Arrangement(Arrangement),
  /// An item alignment.
  ItemAlignment(ItemAlignment),
  /// A list of grid tracks.
  Tracks(Vec<GridTrack<N>>),
  /// A grid flow.
  GridFlow(GridFlow),
  /// A grid placement.
  Placement(Line<GridPlacement>),
  /// An overflow mode.
  Overflow(Overflow),
  /// A visibility.
  Visibility(Visibility),
  /// A value of one of the medium's own properties.
  Extension(X::Value),
}

/// A type a box property holds, erasable into a [`BoxValue`].
///
/// Every shared property type has it for every facade; a medium's own property types
/// implement it for that medium's extension.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a box property type under this extension",
  label = "a box property holds a shared value type, or one of the medium's own"
)]
pub trait IntoBoxValue<N: Scalar, X: StyleExtension>:
  Clone + PartialEq + Debug + Fingerprint + 'static
{
  /// This value, erased.
  fn into_box_value(self) -> BoxValue<N, X>;

  /// This type's reading of an erased value; `None` when it holds another type. A type
  /// made of one length also reads a fixed-length dimension, as a length token resolves
  /// to, as a uniform value.
  fn from_box_value(value: BoxValue<N, X>) -> Option<Self>;
}

impl<N: Scalar, X: StyleExtension> IntoBoxValue<N, X> for Dimension<N> {
  #[inline(always)]
  fn into_box_value(self) -> BoxValue<N, X> {
    BoxValue::Dimension(self)
  }

  #[inline(always)]
  fn from_box_value(value: BoxValue<N, X>) -> Option<Self> {
    match value {
      BoxValue::Dimension(value) => Some(value),
      _ => None,
    }
  }
}

/// The shared property types, each erasing into its own [`BoxValue`] variant and
/// reading it back.
macro_rules! box_values {
  ($($value:ty => $variant:ident $(, $lenient:expr)?);+ $(;)?) => {$(
    impl<N: Scalar, X: StyleExtension> IntoBoxValue<N, X> for $value {
      #[inline(always)]
      fn into_box_value(self) -> BoxValue<N, X> {
        BoxValue::$variant(self)
      }

      #[inline(always)]
      fn from_box_value(value: BoxValue<N, X>) -> Option<Self> {
        match value {
          BoxValue::$variant(value) => Some(value),
          $(BoxValue::Dimension(Dimension::Length(length)) => Some($lenient(length)),)?
          _ => None,
        }
      }
    }
  )+};
}

box_values!(
  f32 => Number;
  Inset<Dimension<N>> => DimensionInset, |length| Inset::all(Dimension::Length(length));
  Inset<N> => Inset, Inset::all;
  Color => Color;
  Border<N> => Border;
  MouseCursor => Cursor;
  Spacing<N> => Spacing, Spacing::all;
  Wrap => Wrap;
  Flexibility<N> => Flexibility;
  Arrangement => Arrangement;
  ItemAlignment => ItemAlignment;
  Vec<GridTrack<N>> => Tracks;
  GridFlow => GridFlow;
  Line<GridPlacement> => Placement;
  Overflow => Overflow;
  Visibility => Visibility;
);

/// One property of a [`BoxStyle`], addressed by type, in whatever shape the style is.
///
/// The shared properties are the types in [`box_fields`]; a medium's extension adds its
/// own and enumerates them through
/// [`StyleExtension::for_each_box_field`].
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a box property",
  label = "a box property is one of the types in `box_fields`, or one the style extension enumerates"
)]
pub trait BoxField<N: Scalar, X: StyleExtension>: 'static {
  /// The type the property holds.
  type Value: IntoBoxValue<N, X>;

  /// The property's name.
  const KEY: PropertyKey<X::Property>;

  /// The field in `style`.
  fn get<S: Shape>(style: &BoxStyle<N, S, X>) -> &S::Of<Self::Value>;

  /// The field in `style`, mutably.
  fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, X>) -> &mut S::Of<Self::Value>;
}

/// Visits a [`BoxStyle`]'s properties by type; the visitor holds whichever styles it
/// reads or writes and reaches each field through [`BoxField`].
pub trait BoxFieldVisitor<N: Scalar, X: StyleExtension> {
  /// Visits the property `F`.
  fn field<F: BoxField<N, X>>(&mut self);
}

/// Visits a [`BoxStyle`]'s fields by value, each with its name.
pub trait BoxVisitor<N: Scalar, S: Shape, X: StyleExtension> {
  /// Visits the field of property `key`, holding a `T`.
  fn value<T: IntoBoxValue<N, X>>(&mut self, key: PropertyKey<X::Property>, value: &S::Of<T>);
}

/// Visits a [`BoxStyle`]'s fields by mutable value, each with its name.
pub trait BoxVisitorMut<N: Scalar, S: Shape, X: StyleExtension> {
  /// Visits the field of property `key`, holding a `T`.
  fn value<T: IntoBoxValue<N, X>>(&mut self, key: PropertyKey<X::Property>, value: &mut S::Of<T>);
}

/// A derived box leaf a split lifted out.
pub type BoxSlot<N, X = ()> =
  DerivedSlot<PropertyKey<<X as StyleExtension>::Property>, BoxValue<N, X>>;

/// The shared box properties as types, and the enumeration walking them.
macro_rules! box_fields {
  ($($field:ident: $value:ty => $property:ident),+ $(,)?) => {
    /// The shared box properties, addressed by type.
    pub mod box_fields {
      use super::{BoxField, BoxStyle, Property, PropertyKey, Scalar, Shape, StyleExtension};

      $(
        #[doc = concat!("The `", stringify!($field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $property;

        impl<N: Scalar, X: StyleExtension> BoxField<N, X> for $property {
          type Value = $value;

          const KEY: PropertyKey<X::Property> = PropertyKey::Shared(Property::$property);

          #[inline(always)]
          fn get<S: Shape>(style: &BoxStyle<N, S, X>) -> &S::Of<$value> {
            &style.$field
          }

          #[inline(always)]
          fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, X>) -> &mut S::Of<$value> {
            &mut style.$field
          }
        }
      )+
    }

    impl<N: Scalar, S: Shape, X: StyleExtension> BoxStyle<N, S, X> {
      /// Visits every property, the shared ones in declaration order and then the
      /// extension's.
      pub fn for_each_field<V: BoxFieldVisitor<N, X>>(visitor: &mut V) {
        $(visitor.field::<box_fields::$property>();)+
        X::for_each_box_field(visitor);
      }
    }
  };
}

box_fields!(
  width: crate::Dimension<N> => Width,
  height: crate::Dimension<N> => Height,
  min_width: crate::Dimension<N> => MinWidth,
  min_height: crate::Dimension<N> => MinHeight,
  max_width: crate::Dimension<N> => MaxWidth,
  max_height: crate::Dimension<N> => MaxHeight,
  aspect_ratio: f32 => AspectRatio,
  margin: crate::Inset<crate::Dimension<N>> => Margin,
  padding: crate::Inset<N> => Padding,
  offset: crate::Inset<crate::Dimension<N>> => Offset,
  pinned: crate::Inset<crate::Dimension<N>> => Pinned,
  color: crate::Color => Color,
  border: crate::Border<N> => Border,
  cursor: crate::MouseCursor => Cursor,
  spacing: crate::Spacing<N> => Spacing,
  wrap: crate::Wrap => Wrap,
  flex: crate::Flexibility<N> => Flex,
  arrangement: crate::Arrangement => Arrangement,
  item_alignment: crate::ItemAlignment => ItemAlignment,
  line_arrangement: crate::Arrangement => LineArrangement,
  item_horizontal_alignment: crate::ItemAlignment => ItemHorizontalAlignment,
  self_alignment: crate::ItemAlignment => SelfAlignment,
  self_horizontal_alignment: crate::ItemAlignment => SelfHorizontalAlignment,
  grid_columns: Vec<crate::GridTrack<N>> => GridColumns,
  grid_rows: Vec<crate::GridTrack<N>> => GridRows,
  grid_implicit_columns: Vec<crate::GridTrack<N>> => GridImplicitColumns,
  grid_implicit_rows: Vec<crate::GridTrack<N>> => GridImplicitRows,
  grid_flow: crate::GridFlow => GridFlow,
  grid_row_placement: crate::Line<crate::GridPlacement> => GridRowPlacement,
  grid_column_placement: crate::Line<crate::GridPlacement> => GridColumnPlacement,
  overflow: crate::Overflow => Overflow,
  visibility: crate::Visibility => Visibility,
);

impl<N: Scalar, S: Shape, X: StyleExtension> BoxStyle<N, S, X> {
  /// Visits every field with its name.
  #[inline(always)]
  pub fn visit<V: BoxVisitor<N, S, X>>(&self, visitor: &mut V) {
    Self::for_each_field(&mut Visit {
      style: self,
      visitor,
    });
  }

  /// Visits every field mutably with its name.
  #[inline(always)]
  pub fn visit_mut<V: BoxVisitorMut<N, S, X>>(&mut self, visitor: &mut V) {
    Self::for_each_field(&mut VisitMut {
      style: self,
      visitor,
    });
  }
}

struct Visit<'a, N: Scalar, S: Shape, X: StyleExtension, V> {
  style: &'a BoxStyle<N, S, X>,
  visitor: &'a mut V,
}

impl<N: Scalar, S: Shape, X: StyleExtension, V: BoxVisitor<N, S, X>> BoxFieldVisitor<N, X>
  for Visit<'_, N, S, X, V>
{
  #[inline(always)]
  fn field<F: BoxField<N, X>>(&mut self) {
    self.visitor.value::<F::Value>(F::KEY, F::get(self.style));
  }
}

struct VisitMut<'a, N: Scalar, S: Shape, X: StyleExtension, V> {
  style: &'a mut BoxStyle<N, S, X>,
  visitor: &'a mut V,
}

impl<N: Scalar, S: Shape, X: StyleExtension, V: BoxVisitorMut<N, S, X>> BoxFieldVisitor<N, X>
  for VisitMut<'_, N, S, X, V>
{
  #[inline(always)]
  fn field<F: BoxField<N, X>>(&mut self) {
    self
      .visitor
      .value::<F::Value>(F::KEY, F::get_mut(self.style));
  }
}

impl<N: Scalar, X: StyleExtension> BoxStyle<N, Plain, X> {
  /// Lays `over` on this style: every property `over` sets replaces this one's.
  #[inline(always)]
  pub fn overlay(&mut self, over: &Self) {
    Self::for_each_field(&mut Overlay { base: self, over });
  }
}

struct Overlay<'a, N: Scalar, X: StyleExtension> {
  base: &'a mut BoxStyle<N, Plain, X>,
  over: &'a BoxStyle<N, Plain, X>,
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for Overlay<'_, N, X> {
  #[inline(always)]
  fn field<F: BoxField<N, X>>(&mut self) {
    if let Some(value) = F::get(self.over) {
      *F::get_mut(self.base) = Some(value.clone());
    }
  }
}

impl<N: Scalar, C: Clone + PartialEq + Debug + 'static, X: StyleExtension>
  BoxStyle<N, Declared<C>, X>
{
  /// The static style a platform receives, and the derived leaves lifted out of it: each
  /// slot's index is its position in the list, and its property's field holds
  /// [`PlatformLeaf::Slot`](super::PlatformLeaf::Slot) where the leaf was.
  #[expect(
    clippy::type_complexity,
    reason = "the style model's own types, plainer spelled out than behind an alias"
  )]
  #[inline(always)]
  pub fn split(self) -> (BoxStyle<N, Static<C>, X>, Vec<BoxSlot<N, X>>) {
    self.split_after(0)
  }

  /// [`split`](Self::split), for a node already holding `taken` slots: each slot's
  /// index is `taken` plus its position in the list.
  #[expect(
    clippy::type_complexity,
    reason = "the style model's own types, plainer spelled out than behind an alias"
  )]
  pub fn split_after(mut self, taken: usize) -> (BoxStyle<N, Static<C>, X>, Vec<BoxSlot<N, X>>) {
    let mut target = BoxStyle::default();
    let mut slots = Vec::new();

    Self::for_each_field(&mut Split {
      source: &mut self,
      target: &mut target,
      slots: &mut slots,
      taken,
    });

    (target, slots)
  }
}

struct Split<'a, N: Scalar, C: Clone + PartialEq + Debug + 'static, X: StyleExtension> {
  source: &'a mut BoxStyle<N, Declared<C>, X>,
  target: &'a mut BoxStyle<N, Static<C>, X>,
  slots: &'a mut Vec<BoxSlot<N, X>>,
  taken: usize,
}

impl<N: Scalar, C: Clone + PartialEq + Debug + 'static, X: StyleExtension> BoxFieldVisitor<N, X>
  for Split<'_, N, C, X>
{
  fn field<F: BoxField<N, X>>(&mut self) {
    let Some(value) = F::get_mut(self.source).take()
    else {
      return;
    };

    let slots = &mut *self.slots;
    let taken = self.taken;
    let split = value.split_derived(|position, compute| {
      let slot =
        Slot(u16::try_from(taken + slots.len()).expect("a node holds at most 65536 derived slots"));

      slots.push(DerivedSlot {
        slot,
        property: F::KEY,
        position,
        compute: Rc::new(move || compute().map(IntoBoxValue::into_box_value)),
      });

      slot
    });

    *F::get_mut(self.target) = Some(split);
  }
}

/// A box's motion: the transitions between its values and the keyframe animations it
/// plays.
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct BoxDynamics<N: Scalar, X: StyleExtension = ()> {
  /// The property transitions.
  pub transitions: Vec<Transition<X::Property>>,
  /// The keyframe animations, each keyframe a style of static leaves.
  pub animations: Vec<Animation<BoxStyle<N, Keyframe, X>>>,
}

impl<N: Scalar, X: StyleExtension> Default for BoxDynamics<N, X> {
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
  use super::{BoxStyle, BoxValue, BoxVisitor, box_fields};
  use crate::{
    BoxField,
    Color,
    Condition,
    Declared,
    Dimension,
    Inset,
    Leaf,
    Media,
    Plain,
    PlatformLeaf,
    Property,
    PropertyKey,
    Shape,
    Slot,
    State,
    StaticLeaf,
    StyleExtension,
    TokenRef,
    Value,
    When,
  };

  type GuiCondition = Condition<f32>;

  /// Splitting keeps every static leaf, replaces each derived one with a numbered slot,
  /// and lists the slots with their property and position.
  #[test]
  fn split_yields_the_static_part_and_the_slot_list() {
    let declared = BoxStyle::<f32, Declared<GuiCondition>> {
      width: Some(Value::from(Dimension::from(10.))),
      padding: Some(Value::from(TokenRef::<Inset<f32>>::new("gutter"))),
      color: Some(
        Value::new(Leaf::derive(|| Color::TRANSPARENT))
          .when(Media::Dark.into(), Color::srgb(0., 0., 0.)),
      ),
      aspect_ratio: Some(1f32.derive_when(State::Hovered.into(), || 2.)),
      ..Default::default()
    };

    let (fixed, slots) = declared.split();

    assert_eq!(
      fixed.width.as_ref().map(|width| width.base().clone()),
      Some(PlatformLeaf::Static(StaticLeaf::Fixed(Dimension::Length(
        10.
      ))))
    );
    assert_eq!(
      fixed.padding.as_ref().map(|padding| padding.base().clone()),
      Some(PlatformLeaf::Static(StaticLeaf::Token(TokenRef::new(
        "gutter"
      ))))
    );
    assert_eq!(fixed.height, None);

    let color = fixed.color.unwrap();

    assert_eq!(color.base(), &PlatformLeaf::Slot(Slot(1)));
    assert_eq!(
      color.variants(),
      &[(
        GuiCondition::Media(Media::Dark),
        PlatformLeaf::Static(StaticLeaf::Fixed(Color::srgb(0., 0., 0.))),
      )]
    );

    let aspect_ratio = fixed.aspect_ratio.unwrap();

    assert_eq!(
      aspect_ratio.base(),
      &PlatformLeaf::Static(StaticLeaf::Fixed(1.))
    );
    assert_eq!(aspect_ratio.variants()[0].1, PlatformLeaf::Slot(Slot(0)));

    assert_eq!(slots.len(), 2, "slots follow declaration order");
    assert_eq!(slots[0].slot, Slot(0));
    assert_eq!(
      slots[0].property,
      PropertyKey::Shared(Property::AspectRatio)
    );
    assert_eq!(slots[0].position, 1);
    assert_eq!(
      (slots[0].compute)(),
      StaticLeaf::Fixed(BoxValue::Number(2.))
    );
    assert_eq!(slots[1].slot, Slot(1));
    assert_eq!(slots[1].property, PropertyKey::Shared(Property::Color));
    assert_eq!(slots[1].position, 0);
    assert_eq!(
      (slots[1].compute)(),
      StaticLeaf::Fixed(BoxValue::Color(Color::TRANSPARENT))
    );
  }

  /// Overlaying replaces exactly the properties the overlay sets.
  #[test]
  fn overlay_later_wins_per_set_field() {
    let mut base = BoxStyle::<u32, Plain>::default();
    let mut over = BoxStyle::<u32, Plain>::default();

    base.width = Some(Dimension::Length(4));
    base.color = Some(Color::srgb(1., 0., 0.));
    over.color = Some(Color::srgb(0., 0., 1.));
    over.padding = Some(Inset::all(1));

    base.overlay(&over);

    assert_eq!(base.width, Some(Dimension::Length(4)));
    assert_eq!(base.color, Some(Color::srgb(0., 0., 1.)));
    assert_eq!(base.padding, Some(Inset::all(1)));
    assert_eq!(base.height, None);
  }

  /// A visitor sees every shared property once, in declaration order, with the fields
  /// a style set.
  #[test]
  fn visit_walks_every_property_in_order() {
    struct Names(Vec<(&'static str, bool)>);

    impl<S: Shape> BoxVisitor<f32, S, ()> for Names {
      fn value<T: super::IntoBoxValue<f32, ()>>(
        &mut self,
        key: PropertyKey<<() as StyleExtension>::Property>,
        value: &S::Of<T>,
      ) {
        self.0.push((key.name(), *value != S::Of::<T>::default()));
      }
    }

    let style = BoxStyle::<f32, Plain> {
      spacing: Some(8f32.into()),
      ..Default::default()
    };
    let mut names = Names(Vec::new());

    style.visit(&mut names);

    assert_eq!(names.0.len(), 32);
    assert_eq!(names.0[0], ("width", false));
    assert_eq!(names.0[14], ("spacing", true));
    assert_eq!(names.0[30], ("overflow", false));
    assert_eq!(names.0[31], ("visibility", false));
    assert_eq!(
      <box_fields::Spacing as BoxField<f32, ()>>::KEY,
      PropertyKey::Shared(Property::Spacing)
    );
  }
}
