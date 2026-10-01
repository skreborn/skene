//! The surface every facade shares over its platform.

/// Declares a facade's shared surface over the platform `$platform`, measuring lengths
/// in `$scalar`, which `$unit` names in the documentation.
///
/// The traits a view and a token set of the platform meet, an alias per built-in view,
/// the value vocabulary in the unit, and `derived`, `reconcile` and `empty`. A facade
/// adds its platform selection, its medium and its entry points around it, and names
/// `skene-foundation` and `skene-reactive` among its dependencies.
#[macro_export]
macro_rules! facade {
  ($platform:ty, $scalar:ty, $unit:literal) => {
    /// The erased token definition the active platform stores per token.
    pub type TokenValue = <$platform as $crate::Platform>::TokenValue;

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
    /// A run of text on its own line; see [`skene_foundation::Text`].
    pub type Text = $crate::Text<$platform>;
    /// A run inside a paragraph; see [`skene_foundation::Span`].
    pub type Span = $crate::Span<$platform>;
    /// A paragraph of spans; see [`skene_foundation::RichText`].
    pub type RichText = $crate::RichText<$platform>;
    /// A branch shown or hidden by a condition; see [`skene_foundation::Show`].
    pub type Show<C = $crate::NoChild> = $crate::Show<$platform, C>;
    /// The view a `set_key` named, built under a keyed scope; see
    /// [`skene_foundation::Keyed`].
    pub type Keyed<V> = $crate::Keyed<$platform, V>;
    /// One of several branches, by key; see [`skene_foundation::Switch`].
    pub type Switch<K> = $crate::Switch<$platform, K>;
    /// Fallback content while a subtree's async work is pending; see
    /// [`skene_foundation::Suspense`].
    pub type Suspense<C = $crate::NoChild> = $crate::Suspense<$platform, C>;
    /// The form a signal makes possible over the one that needs none; see
    /// [`skene_foundation::Enhance`].
    pub type Enhance<C = $crate::NoChild> = $crate::Enhance<$platform, C>;
    /// Fallback content when a subtree reports an error; see
    /// [`skene_foundation::ErrorBoundary`].
    pub type ErrorBoundary<C = $crate::NoChild> =
      $crate::ErrorBoundary<$platform, C>;
    /// The one theme view, defining the token set `S` beneath it; see
    /// [`skene_foundation::Theme`].
    pub type Theme<S, C = $crate::NoChild> = $crate::Theme<$platform, S, C>;
    /// The one control builder over a behavior; see [`skene_foundation::Control`].
    pub type Control<B, C = $crate::NoChild> = $crate::Control<$platform, B, C>;
    /// A built control, for a behavior to act on; see [`skene_foundation::ControlHandle`].
    pub type ControlHandle = $crate::ControlHandle<$platform>;
    /// A control's label; see [`skene_foundation::Label`].
    pub type Label = $crate::Label<$platform>;
    /// A button; see [`skene_foundation::BasicButton`].
    pub type BasicButton<C = $crate::NoChild> = $crate::BasicButton<$platform, C>;
    /// A checkbox; see [`skene_foundation::BasicCheckbox`].
    pub type BasicCheckbox<C = $crate::NoChild> = $crate::BasicCheckbox<$platform, C>;
    /// A slider; see [`skene_foundation::BasicSlider`].
    pub type BasicSlider<C = $crate::NoChild> = $crate::BasicSlider<$platform, C>;
    /// A radio group; see [`skene_foundation::BasicRadioGroup`].
    pub type BasicRadioGroup<C = $crate::NoChild> =
      $crate::BasicRadioGroup<$platform, C>;
    /// A radio inside a group; see [`skene_foundation::BasicRadio`].
    pub type BasicRadio<C = $crate::NoChild> = $crate::BasicRadio<$platform, C>;
    /// A single-line text entry; see [`skene_foundation::BasicTextField`].
    pub type BasicTextField<C = $crate::NoChild> =
      $crate::BasicTextField<$platform, C>;
    /// A multi-line text entry; see [`skene_foundation::BasicTextEditor`].
    pub type BasicTextEditor<C = $crate::NoChild> =
      $crate::BasicTextEditor<$platform, C>;
    /// A listbox; see [`skene_foundation::BasicListbox`].
    pub type BasicListbox<C = $crate::NoChild> = $crate::BasicListbox<$platform, C>;
    /// An option inside a listbox or a picker; see [`skene_foundation::BasicListboxOption`].
    pub type BasicListboxOption<C = $crate::NoChild> = $crate::BasicListboxOption<$platform, C>;
    /// A picker: a popup face, an overlay and a listbox; see
    /// [`skene_foundation::BasicPicker`].
    pub type BasicPicker<F = $crate::NoChild> = $crate::BasicPicker<$platform, F>;
    /// A picker's face; see [`skene_foundation::Face`].
    pub type Face = $crate::Face<$platform>;
    /// Content above the app; see [`skene_foundation::Overlay`].
    pub type Overlay<C = $crate::NoChild> = $crate::Overlay<$platform, C>;
    /// Content shown beside the view it describes; see [`skene_foundation::Tooltip`].
    pub type Tooltip<C = $crate::NoChild> = $crate::Tooltip<$platform, C>;
    /// Commands shown beside the pointer on a secondary press over the view it serves;
    /// see [`skene_foundation::ContextMenu`].
    pub type ContextMenu<C = $crate::NoChild> = $crate::ContextMenu<$platform, C>;
    /// Keyboard shortcuts over the view it serves; see [`skene_foundation::Keymap`].
    pub type Keymap<C = $crate::NoChild> = $crate::Keymap<$platform, C>;
    /// A menu; see [`skene_foundation::BasicMenu`].
    pub type BasicMenu<C = $crate::NoChild> = $crate::BasicMenu<$platform, C>;
    /// One item within a menu; see [`skene_foundation::BasicMenuItem`].
    pub type BasicMenuItem<C = $crate::NoChild> = $crate::BasicMenuItem<$platform, C>;
    /// A handle naming the node a view built; see [`skene_foundation::NodeHandle`].
    pub type NodeHandle = $crate::NodeHandle<$platform>;
    /// A box drawn by a paint callback; see [`skene_foundation::Painted`].
    pub type Painted = $crate::Painted<$platform>;
    /// A scrolling keyed list building only the slice in view; see
    /// [`skene_foundation::LazyColumn`].
    pub type LazyColumn = $crate::LazyColumn<$platform>;
    /// A scrolling keyed row building only the slice in view; see
    /// [`skene_foundation::LazyRow`].
    pub type LazyRow = $crate::LazyRow<$platform>;
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
