//! The record: properties under Skene's uniform names, unresolved.

use std::{fmt, marker::PhantomData};

use crate::{
  Color, FontWeight, Inset, Property, Value,
  seam::style::{CompileCx, KeptValue},
  value::{
    Alignment, Arrangement, AxisAlignment, Border, Corners, Curve, Dimension, Edges, Flexibility,
    Flow, FontFamily, FontStyle, GridPlacement, Offset, Ring, Shadows, Spacing, TextAlignment,
    TextDecoration, TextDecorationStyle, Tracks,
  },
};

/// A type a property of the record holds.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a type a property holds",
  label = "not a property's type",
  note = "a property holds one of the record's kinds: a color, a number, a count, a flag, or one of the style model's value types"
)]
pub trait Kinded: Property + PartialEq + fmt::Debug {
  /// Its kind.
  const KIND: Kind;

  /// A declared value of this type, erased.
  fn declare(value: Value<Self>) -> Declared;

  /// A resolved value of this type, erased.
  fn erase(self) -> Resolved;

  /// The value `resolved` holds, where it is of this type.
  fn unerase(resolved: &Resolved) -> Option<Self>;
}

/// The kinds of the record's values, with their erased declared and resolved forms.
macro_rules! kinds {
  ($($(#[doc = $doc:literal])* $kind:ident($ty:ty),)*) => {
    /// The type a property holds.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub enum Kind {
      $($(#[doc = $doc])* $kind,)*
    }

    /// A property's declared value, unresolved: fixed, token and derived leaves under
    /// conditions.
    #[derive(Clone)]
    pub enum Declared {
      $($(#[doc = $doc])* $kind(Value<$ty>),)*
    }

    /// A property's value, resolved.
    #[derive(Clone, PartialEq, Debug)]
    pub enum Resolved {
      $($(#[doc = $doc])* $kind($ty),)*
    }

    $(
      impl Kinded for $ty {
        const KIND: Kind = Kind::$kind;

        #[inline(always)]
        fn declare(value: Value<Self>) -> Declared {
          Declared::$kind(value)
        }

        #[inline(always)]
        fn erase(self) -> Resolved {
          Resolved::$kind(self)
        }

        #[inline(always)]
        fn unerase(resolved: &Resolved) -> Option<Self> {
          match resolved {
            Resolved::$kind(value) => Some(value.clone()),
            _ => None,
          }
        }
      }
    )*

    impl Declared {
      /// The kind of value declared.
      #[must_use]
      pub const fn kind(&self) -> Kind {
        match self {
          $(Self::$kind(_) => Kind::$kind,)*
        }
      }

      /// Compiled to the level `cx` keeps, subscribing the running effect to what it read.
      pub(crate) fn compile(&self, cx: &mut CompileCx<'_>) -> Option<KeptValue> {
        match self {
          $(Self::$kind(value) => cx.value(value),)*
        }
      }
    }
  };
}

kinds! {
  /// A color.
  Color(Color),
  /// A length in pixels, or a plain number.
  Number(f32),
  /// A count.
  Count(u32),
  /// A flag.
  Flag(bool),
  /// Space inside a box's edges.
  Inset(Inset),
  /// A size along an axis.
  Dimension(Dimension),
  /// Space between neighbors.
  Spacing(Spacing),
  /// A distribution along an axis.
  Arrangement(Arrangement),
  /// An alignment along one axis.
  AxisAlignment(AxisAlignment),
  /// An alignment along both axes.
  Alignment(Alignment),
  /// Grid tracks.
  Tracks(Tracks),
  /// A grid's placement order.
  Flow(Flow),
  /// Grow and shrink weights.
  Flexibility(Flexibility),
  /// Pinned edges.
  Edges(Edges),
  /// A grid cell.
  GridPlacement(GridPlacement),
  /// Corner radii.
  Corners(Corners),
  /// A border.
  Border(Border),
  /// A ring around a box.
  Ring(Ring),
  /// A box moved from where it was laid out.
  Offset(Offset),
  /// Shadows.
  Shadows(Shadows),
  /// A font weight.
  FontWeight(FontWeight),
  /// A font style.
  FontStyle(FontStyle),
  /// A typeface.
  FontFamily(FontFamily),
  /// Decoration lines.
  TextDecoration(TextDecoration),
  /// How decoration lines are drawn.
  TextDecorationStyle(TextDecorationStyle),
  /// A paragraph's alignment.
  TextAlignment(TextAlignment),
}

/// Which part of the seam consumes a property.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Group {
  /// Painted around and behind a box.
  Box,
  /// Inherited by the text inside a node.
  Run,
  /// A layout container's or a node's own layout.
  Layout,
  /// Folded in from a wrapper (`Flexible`, `Pinned`, `GridCell`): the parent's layout reads
  /// it.
  Placement,
}

/// The record's names and kinds, and a typed key per name.
macro_rules! props {
  ($($group:ident { $($(#[doc = $doc:literal])* $name:ident, $key:ident: $ty:ty;)* })*) => {
    /// A property's uniform name: the same on every platform.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
    pub enum Prop {
      $($($(#[doc = $doc])* $name,)*)*
    }

    impl Prop {
      /// Every name.
      pub const ALL: &[Self] = &[$($(Self::$name,)*)*];

      /// Which part of the seam consumes it.
      #[must_use]
      pub const fn group(self) -> Group {
        match self {
          $($(Self::$name => Group::$group,)*)*
        }
      }

      /// The kind of value it holds.
      #[must_use]
      pub const fn kind(self) -> Kind {
        match self {
          $($(Self::$name => <$ty as Kinded>::KIND,)*)*
        }
      }
    }

    /// A typed key per name: `record.set(keys::BACKGROUND, palette.card)`.
    pub mod keys {
      use super::*;

      $($($(#[doc = $doc])* pub const $key: Key<$ty> = Key::new(Prop::$name);)*)*
    }
  };
}

props! {
  Box {
    /// The fill behind a box's content.
    Background, BACKGROUND: Color;
    /// A line around the box.
    Border, BORDER: Border;
    /// The color of the line around the box, the box's text color where unset.
    BorderColor, BORDER_COLOR: Color;
    /// The corners' radii.
    CornerRadius, CORNER_RADIUS: Corners;
    /// The shadows the box casts.
    Shadow, SHADOW: Shadows;
    /// The box's opacity, 0 to 1.
    Opacity, OPACITY: f32;
    /// Whether content outside the box's edges is cut off.
    Clip, CLIP: bool;
    /// A line around the box that takes no room (a focus ring).
    Ring, RING: Ring;
    /// The ring's color, the box's text color where unset.
    RingColor, RING_COLOR: Color;
    /// How far the box and everything inside it are drawn from where they were laid out.
    Offset, OFFSET: Offset;
  }
  Run {
    /// The text's color.
    Color, COLOR: Color;
    /// The typeface.
    FontFamily, FONT_FAMILY: FontFamily;
    /// The font's size, in pixels.
    FontSize, FONT_SIZE: f32;
    /// The font's weight.
    FontWeight, FONT_WEIGHT: FontWeight;
    /// Upright or slanted.
    FontStyle, FONT_STYLE: FontStyle;
    /// The height of a line, as a multiple of the font size.
    LineHeight, LINE_HEIGHT: f32;
    /// Extra space between letters, in pixels.
    LetterSpacing, LETTER_SPACING: f32;
    /// The decoration lines.
    Decoration, DECORATION: TextDecoration;
    /// How the decoration lines are drawn.
    DecorationStyle, DECORATION_STYLE: TextDecorationStyle;
    /// The decoration lines' color, the text's where unset.
    DecorationColor, DECORATION_COLOR: Color;
    /// Where a paragraph's lines sit.
    TextAlignment, TEXT_ALIGNMENT: TextAlignment;
    /// The most lines a paragraph shows, the rest elided.
    MaxLines, MAX_LINES: u32;
    /// A text entry's caret, the text's color where unset.
    CaretColor, CARET_COLOR: Color;
    /// The fill behind a text entry's selection, the platform's where unset.
    SelectionColor, SELECTION_COLOR: Color;
    /// An empty text entry's placeholder, the platform's muted ink where unset.
    PlaceholderColor, PLACEHOLDER_COLOR: Color;
  }
  Layout {
    /// The width.
    Width, WIDTH: Dimension;
    /// The height.
    Height, HEIGHT: Dimension;
    /// The least width.
    MinWidth, MIN_WIDTH: Dimension;
    /// The least height.
    MinHeight, MIN_HEIGHT: Dimension;
    /// The greatest width.
    MaxWidth, MAX_WIDTH: Dimension;
    /// The greatest height.
    MaxHeight, MAX_HEIGHT: Dimension;
    /// Space inside the box's edges.
    Padding, PADDING: Inset;
    /// Space between neighboring children.
    Spacing, SPACING: Spacing;
    /// How a row or a column distributes its children along its axis.
    Arrangement, ARRANGEMENT: Arrangement;
    /// Where a row's or a column's children sit across its axis.
    ItemAlignment, ITEM_ALIGNMENT: AxisAlignment;
    /// Where a container's, a stack's or a grid's children sit in their cells.
    Alignment, ALIGNMENT: Alignment;
    /// How a wrapping row distributes its lines.
    LineArrangement, LINE_ARRANGEMENT: Arrangement;
    /// Whether a row wraps its children onto more lines.
    Wrap, WRAP: bool;
    /// A grid's columns.
    Columns, COLUMNS: Tracks;
    /// A grid's rows.
    Rows, ROWS: Tracks;
    /// The columns a grid adds as it places children.
    ImplicitColumns, IMPLICIT_COLUMNS: Tracks;
    /// The rows a grid adds as it places children.
    ImplicitRows, IMPLICIT_ROWS: Tracks;
    /// The order a grid places children in.
    Flow, FLOW: Flow;
  }
  Placement {
    /// How a child of a row or a column grows and shrinks (`Flexible`).
    Flexibility, FLEXIBILITY: Flexibility;
    /// Where a child sits in its own cell or across its row or column.
    SelfAlignment, SELF_ALIGNMENT: Alignment;
    /// The edges a child of a stack is pinned to (`Pinned`).
    Pin, PIN: Edges;
    /// A child's cell in a grid (`GridCell`).
    Cell, CELL: GridPlacement;
  }
}

/// A typed key: a uniform name and the type its values hold.
pub struct Key<T> {
  prop: Prop,
  ty: PhantomData<fn() -> T>,
}

impl<T> Clone for Key<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for Key<T> {}

impl<T> Key<T> {
  /// The key of `prop`.
  #[inline(always)]
  #[must_use]
  pub const fn new(prop: Prop) -> Self {
    Self {
      prop,
      ty: PhantomData,
    }
  }

  /// Its name.
  #[inline(always)]
  #[must_use]
  pub const fn prop(self) -> Prop {
    self.prop
  }
}

/// A transition: a property's change animated over a duration.
///
/// A box's or a text's own look animates (its colors, lines, corners, shadows, opacity and
/// font metrics); a layout property, and one the platform cannot interpolate (a typeface, a
/// slant, which lines decorate a text), changes at once.
#[derive(Clone, Copy, PartialEq, Debug)]
#[must_use]
pub struct Transition {
  /// The property that animates.
  pub prop: Prop,
  /// How long a change takes, in milliseconds.
  pub duration: u32,
  /// How long a change waits to start, in milliseconds.
  pub delay: u32,
  /// The easing curve.
  pub curve: Curve,
}

impl Transition {
  /// Changes of the property `key` names, animated over `duration` milliseconds: at once,
  /// slow at both ends, unless told otherwise.
  #[inline(always)]
  pub const fn new<T>(key: Key<T>, duration: u32) -> Self {
    Self {
      prop: key.prop(),
      duration,
      delay: 0,
      curve: Curve::EaseInOut,
    }
  }

  /// How long a change waits to start, in milliseconds; none where unset.
  #[inline(always)]
  pub const fn set_delay(mut self, delay: u32) -> Self {
    self.delay = delay;

    self
  }

  /// The easing curve; slow at both ends where unset.
  #[inline(always)]
  pub const fn set_curve(mut self, curve: Curve) -> Self {
    self.curve = curve;

    self
  }
}

/// A node's properties under their uniform names, unresolved, and its transitions: what
/// crosses the seam for box, run and layout properties.
///
/// A wrapper's properties are folded into its child's record before the child is created
/// ([`Record::fold`]), so a wrapper realizes nothing.
#[derive(Clone, Default)]
pub struct Record {
  entries: Vec<(Prop, Declared)>,
  transitions: Vec<Transition>,
}

impl fmt::Debug for Record {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_list()
      .entries(self.entries.iter().map(|(prop, _)| prop))
      .finish()
  }
}

impl Record {
  /// An empty record.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      entries: Vec::new(),
      transitions: Vec::new(),
    }
  }

  /// Sets the property `key` names, replacing what it held.
  pub fn set<T: Kinded>(&mut self, key: Key<T>, value: impl Into<Value<T>>) -> &mut Self {
    self.put(key.prop, T::declare(value.into()))
  }

  /// Sets `prop` to an already erased value of its kind.
  ///
  /// # Panics
  ///
  /// In a debug build, where the value's kind is not the property's.
  pub fn put(&mut self, prop: Prop, value: Declared) -> &mut Self {
    debug_assert_eq!(
      prop.kind(),
      value.kind(),
      "{prop:?} holds {:?}",
      prop.kind()
    );

    match self.entries.iter_mut().find(|(name, _)| *name == prop) {
      Some(entry) => entry.1 = value,
      None => self.entries.push((prop, value)),
    }

    self
  }

  /// Adds a transition, replacing one of the same property.
  pub fn add_transition(&mut self, transition: Transition) -> &mut Self {
    self
      .transitions
      .retain(|existing| existing.prop != transition.prop);
    self.transitions.push(transition);

    self
  }

  /// Folds a wrapper's record in: its properties and transitions join this one's.
  pub fn fold(&mut self, wrapper: &Self) -> &mut Self {
    for (prop, value) in &wrapper.entries {
      self.put(*prop, value.clone());
    }

    for transition in &wrapper.transitions {
      self.add_transition(*transition);
    }

    self
  }

  /// Each property with its declared value, in the order set.
  pub fn entries(&self) -> impl Iterator<Item = &(Prop, Declared)> {
    self.entries.iter()
  }

  /// The transitions.
  #[must_use]
  pub fn transitions(&self) -> &[Transition] {
    &self.transitions
  }

  /// Whether the record holds a property of `group`.
  #[must_use]
  pub fn has_group(&self, group: Group) -> bool {
    self.entries.iter().any(|(prop, _)| prop.group() == group)
  }
}
