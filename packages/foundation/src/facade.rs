//! The surface every facade shares over its platform.

/// Declares a facade's shared surface over the platform `$platform`, measuring lengths
/// in `$scalar`, which `$unit` names in the documentation.
///
/// The traits a view and a token set of the platform meet, an alias for each building
/// block that is no view of its own (the erased view, the layout box, a keyed view, a
/// control's handle and label, a picker's face, a node handle and a child builder), the
/// value vocabulary in the unit, and `derived`, `reconcile` and `empty`. A facade
/// declares its views around it as newtypes the `impl_*!` macros implement, adds its
/// platform selection, its medium and its entry points, and names `skene-foundation`
/// and `skene-reactive` among its dependencies.
#[macro_export]
macro_rules! facade {
  ($platform:ty, $scalar:ty, $unit:literal) => {
    /// The erased token definition the active platform stores per token.
    pub type TokenValue = <$platform as $crate::Realization>::TokenValue;

    /// A view on the active platform, accepted as a child of every kind; see
    /// [`skene_foundation::View`].
    pub trait View: $crate::View<$platform> {
      /// Realizes this view and returns its root node.
      #[inline(always)]
      fn build(self) -> $crate::NodeId
      where
        Self: Sized,
      {
        $crate::View::build(self)
      }

      /// Erases this view's concrete type.
      #[inline(always)]
      fn into_any(self) -> AnyView
      where
        Self: Sized,
      {
        $crate::View::into_any(self)
      }
    }

    impl<V: $crate::View<$platform>> View for V {}

    /// A token set read from the theme in force on the active platform; see
    /// [`skene_foundation::ThemedIn`].
    ///
    /// Every [`TokenSet`](skene_foundation::TokenSet) with a default has it:
    /// `SimplePalette::current()` reads the nearest theme's palette, or the default when
    /// no theme is above.
    pub trait ThemedIn: $crate::ThemedIn<$platform> {
      /// The set the nearest theme provides, or the default without one.
      #[inline(always)]
      #[must_use]
      fn current() -> Self {
        <Self as $crate::ThemedIn<$platform>>::current()
      }

      /// The nearest theme's set as a memo, or a memo of the default without one.
      #[inline(always)]
      fn watch() -> ::skene_reactive::Memo<Self> {
        <Self as $crate::ThemedIn<$platform>>::watch()
      }
    }

    impl<S: $crate::ThemedIn<$platform>> ThemedIn for S {}

    /// A type-erased view.
    pub type AnyView = $crate::AnyView<$platform>;
    /// The box every layout view is built from, accepting children of `Kind`; see
    /// [`skene_foundation::LayoutBox`].
    pub type LayoutBox<Kind> = $crate::LayoutBox<$platform, Kind>;
    /// The view a `set_key` named, built under a keyed scope; see
    /// [`skene_foundation::Keyed`].
    pub type Keyed<V> = $crate::Keyed<$platform, V>;
    /// A built control, for a behavior to act on; see [`skene_foundation::ControlHandle`].
    pub type ControlHandle = $crate::ControlHandle<$platform>;
    /// A control's label; see [`skene_foundation::Label`].
    pub type Label = $crate::Label<$platform>;
    /// A picker's face; see [`skene_foundation::Face`].
    pub type Face = $crate::Face<$platform>;
    /// A handle naming the node a view built; see [`skene_foundation::NodeHandle`].
    pub type NodeHandle = $crate::NodeHandle<$platform>;
    /// The closure building a view's child under the context the view provides; see
    /// [`skene_foundation::ChildBuilder`].
    pub type ChildBuilder = $crate::ChildBuilder<$platform>;

    #[doc = concat!("A condition on the environment, in ", $unit, "; see [`skene_foundation::Media`].")]
    pub type Media = $crate::Media<$scalar>;
    /// A condition a property varies under; see [`skene_foundation::Condition`].
    pub type Condition = $crate::Condition<$scalar>;
    #[doc = concat!("A test of a container's size, in ", $unit, "; see [`skene_foundation::ContainerQuery`].")]
    pub type ContainerQuery = $crate::ContainerQuery<$scalar>;
    /// A property's value: a leaf under conditions; see [`skene_foundation::Value`].
    pub type Value<T> = $crate::Value<T, Condition>;
    /// A token's definition; see [`skene_foundation::Definition`].
    pub type Definition<T> = $crate::Definition<T, $scalar>;
    /// A token as a palette holds it; see [`skene_foundation::Token`].
    pub type Token<T> = $crate::Token<T, $scalar>;
    #[doc = concat!("Spacing on the four edges of a box, in ", $unit, ".")]
    pub type Inset<T = $scalar> = $crate::Inset<T>;
    #[doc = concat!("A dimension, in ", $unit, " or as a fraction of the parent.")]
    pub type Dimension = $crate::Dimension<$scalar>;
    #[doc = concat!("Per-axis spacing between children, in ", $unit, ".")]
    pub type Spacing = $crate::Spacing<$scalar>;
    #[doc = concat!("A 2D size, in ", $unit, ".")]
    pub type Size = $crate::Size<$scalar>;
    #[doc = concat!("A point, in ", $unit, ".")]
    pub type Point = $crate::Point<$scalar>;
    #[doc = concat!("A positioned, sized rectangle, in ", $unit, ".")]
    pub type Rect = $crate::Rect<$scalar>;

    /// Mounts a reactive subtree between anchors, rebuilt whenever a signal `build`
    /// reads changes; see [`skene_foundation::derived`].
    pub fn derived(build: impl Fn() -> $crate::NodeId + 'static) -> $crate::NodeId {
      $crate::derived::<$platform>(build)
    }

    /// Reactively reconciles the content of the segment `start` begins from a keyed
    /// collection; see [`skene_foundation::reconcile`].
    pub fn reconcile<T, I, K>(
      start: $crate::NodeId,
      items: impl Fn() -> I + 'static,
      key: impl Fn(&T) -> K + 'static,
      build: impl Fn(T) -> $crate::NodeId + 'static,
    ) where
      I: IntoIterator<Item = T>,
      K: Eq + ::std::hash::Hash + 'static,
    {
      $crate::reconcile::<$platform, T, I, K>(start, items, key, build);
    }

    /// A view that renders nothing.
    #[inline(always)]
    #[must_use]
    pub fn empty() -> impl View {
      $crate::empty::<$platform>()
    }
  };
}
