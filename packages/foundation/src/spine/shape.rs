//! What a style's fields hold, by the stage the style is at.

use std::{fmt::Debug, marker::PhantomData};

use super::{Conditional, PlatformLeaf, StaticLeaf, Value};

/// The stage a style is at, fixing what each of its fields holds.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a style shape",
  label = "a style is `Declared` by a builder, `Static` for a platform, a `Keyframe` of an animation, or `Plain`"
)]
pub trait Shape: Copy + Default + PartialEq + Debug + 'static {
  /// A field holding a `T`.
  type Of<T: Clone + PartialEq + Debug + 'static>: Clone + PartialEq + Debug + Default;
}

/// What a builder collects: a [`Value`] per set property, conditional over `C` and
/// possibly derived.
pub struct Declared<C>(PhantomData<fn() -> C>);

impl<C: Clone + PartialEq + Debug + 'static> Shape for Declared<C> {
  type Of<T: Clone + PartialEq + Debug + 'static> = Option<Value<T, C>>;
}

/// What a platform receives: a conditional [`PlatformLeaf`] per set property, every derived
/// leaf replaced by the slot a render effect writes.
pub struct Static<C>(PhantomData<fn() -> C>);

impl<C: Clone + PartialEq + Debug + 'static> Shape for Static<C> {
  type Of<T: Clone + PartialEq + Debug + 'static> = Option<Conditional<PlatformLeaf<T>, C>>;
}

/// A plain value per set property: a style resolved against live state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Plain;

impl Shape for Plain {
  type Of<T: Clone + PartialEq + Debug + 'static> = Option<T>;
}

/// A static leaf per set property: an animation keyframe, each leaf a fixed value, a
/// token or an expression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Keyframe;

impl Shape for Keyframe {
  type Of<T: Clone + PartialEq + Debug + 'static> = Option<StaticLeaf<T>>;
}

/// The marker impls a shape needs, free of bounds on the condition it carries.
macro_rules! marker_impls {
  ($($marker:ident),+ $(,)?) => {$(
    impl<C> Clone for $marker<C> {
      #[inline(always)]
      fn clone(&self) -> Self {
        *self
      }
    }

    impl<C> Copy for $marker<C> {}

    impl<C> Default for $marker<C> {
      #[inline(always)]
      fn default() -> Self {
        Self(PhantomData)
      }
    }

    impl<C> PartialEq for $marker<C> {
      #[inline(always)]
      fn eq(&self, _: &Self) -> bool {
        true
      }
    }

    impl<C> Eq for $marker<C> {}

    impl<C> Debug for $marker<C> {
      fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(stringify!($marker))
      }
    }
  )+};
}

marker_impls!(Declared, Static);
