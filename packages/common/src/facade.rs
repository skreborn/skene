//! The surface every facade shares over its platform.

/// Declares a facade's shared surface over the platform `$platform`, measuring lengths
/// in `$scalar`, which `$unit` names in the documentation.
///
/// The traits a view and a token set of the platform meet, an alias per built-in view,
/// the value vocabulary in the unit, and `derived`, `reconcile` and `empty`. A facade
/// adds its platform selection, its medium and its entry points around it, and names
/// `skene-core` and `skene-reactive` among its dependencies.
#[macro_export]
macro_rules! facade {
  ($platform:ty, $scalar:ty, $unit:literal) => {
    /// The erased token definition the active platform stores per token.
    pub type TokenValue = <$platform as ::skene_core::Platform>::TokenValue;

    /// A view on the active platform; see [`skene_core::View`].
    pub trait View: ::skene_core::View<$platform> {
      /// Realizes this view and returns its root node.
      #[inline(always)]
      fn build(self) -> ::skene_core::NodeId
      where
        Self: Sized,
      {
        ::skene_core::View::build(self)
      }

      /// Erases this view's concrete type.
      #[inline(always)]
      fn into_any(self) -> AnyView
      where
        Self: Sized,
      {
        ::skene_core::View::into_any(self)
      }
    }

    impl<V: ::skene_core::View<$platform>> View for V {}

    /// A value that can be turned into a view on the active platform, accepted as a
    /// child of every kind; see [`skene_core::IntoView`] and
    /// [`AnyKind`](skene_core::AnyKind).
    pub trait IntoView: ::skene_core::IntoView<$platform> + ::skene_core::AnyKind {
      /// Converts this value into a view.
      #[inline(always)]
      fn into_view(self) -> AnyView
      where
        Self: Sized,
      {
        ::skene_core::IntoView::into_view(self)
      }
    }

    impl<V: ::skene_core::IntoView<$platform> + ::skene_core::AnyKind> IntoView for V {}

    /// A token set read from the theme in force on the active platform; see
    /// [`skene_common::ThemedIn`].
    ///
    /// Every [`TokenSet`](skene_core::TokenSet) with a default has it:
    /// `SimplePalette::current()` reads the nearest theme's palette, or the default when
    /// no theme is above.
    pub trait ThemedIn: ::skene_common::ThemedIn<$platform> {
      /// The set the nearest theme provides, or the default without one.
      #[inline(always)]
      #[must_use]
      fn current() -> Self {
        <Self as ::skene_common::ThemedIn<$platform>>::current()
      }

      /// The nearest theme's set as a memo, or a memo of the default without one.
      #[inline(always)]
      fn watch() -> ::skene_reactive::Memo<Self> {
        <Self as ::skene_common::ThemedIn<$platform>>::watch()
      }
    }

    impl<S: ::skene_common::ThemedIn<$platform>> ThemedIn for S {}

    /// A type-erased view.
    pub type AnyView = ::skene_core::AnyView<$platform>;
    /// The one freely-decorated box around exactly one child; see
    /// [`skene_common::Container`].
    pub type Container = ::skene_common::Container<$platform>;
    /// A horizontal flex layout; see [`skene_common::Row`].
    pub type Row = ::skene_common::Row<$platform>;
    /// A vertical flex layout; see [`skene_common::Column`].
    pub type Column = ::skene_common::Column<$platform>;
    /// A grid layout; see [`skene_common::Grid`].
    pub type Grid = ::skene_common::Grid<$platform>;
    /// A stack layout; see [`skene_common::Stack`].
    pub type Stack = ::skene_common::Stack<$platform>;
    /// The box every layout view is built from, accepting children of `Kind`; see
    /// [`skene_common::LayoutBox`].
    pub type LayoutBox<Kind> = ::skene_common::LayoutBox<$platform, Kind>;
    /// A flex child's grow, shrink and initial size; see [`skene_common::Flexible`].
    pub type Flexible<C = ::skene_core::NoChild> = ::skene_common::Flexible<$platform, C>;
    /// A stack child pinned to its edges; see [`skene_common::Pinned`].
    pub type Pinned<C = ::skene_core::NoChild> = ::skene_common::Pinned<$platform, C>;
    /// A grid child's placement; see [`skene_common::GridCell`].
    pub type GridCell<C = ::skene_core::NoChild> = ::skene_common::GridCell<$platform, C>;
    /// A run of text on its own line; see [`skene_common::Text`].
    pub type Text = ::skene_common::Text<$platform>;
    /// A run inside a paragraph; see [`skene_common::Span`].
    pub type Span = ::skene_common::Span<$platform>;
    /// A paragraph of spans; see [`skene_common::RichText`].
    pub type RichText = ::skene_common::RichText<$platform>;
    /// A branch shown or hidden by a condition; see [`skene_common::Show`].
    pub type Show<C = ::skene_core::NoChild> = ::skene_common::Show<$platform, C>;
    /// The view a `set_key` named, built under a keyed scope; see
    /// [`skene_common::Keyed`].
    pub type Keyed<V> = ::skene_common::Keyed<$platform, V>;
    /// One of several branches, by key; see [`skene_common::Switch`].
    pub type Switch<K> = ::skene_common::Switch<$platform, K>;
    /// Fallback content while a subtree's async work is pending; see
    /// [`skene_common::Suspense`].
    pub type Suspense<C = ::skene_core::NoChild> = ::skene_common::Suspense<$platform, C>;
    /// The form a signal makes possible over the one that needs none; see
    /// [`skene_common::Enhance`].
    pub type Enhance<C = ::skene_core::NoChild> = ::skene_common::Enhance<$platform, C>;
    /// Fallback content when a subtree reports an error; see
    /// [`skene_common::ErrorBoundary`].
    pub type ErrorBoundary<C = ::skene_core::NoChild> =
      ::skene_common::ErrorBoundary<$platform, C>;
    /// The one theme view, defining the token set `S` beneath it; see
    /// [`skene_common::Theme`].
    pub type Theme<S, C = ::skene_core::NoChild> = ::skene_common::Theme<$platform, S, C>;
    /// The one control builder over a behavior; see [`skene_common::Control`].
    pub type Control<B, C = ::skene_core::NoChild> = ::skene_common::Control<$platform, B, C>;
    /// A built control, for a behavior to act on; see [`skene_common::ControlHandle`].
    pub type ControlHandle = ::skene_common::ControlHandle<$platform>;
    /// A control's label; see [`skene_common::Label`].
    pub type Label = ::skene_common::Label<$platform>;
    /// A button; see [`skene_common::RawButton`].
    pub type RawButton<C = ::skene_core::NoChild> = ::skene_common::RawButton<$platform, C>;
    /// A checkbox; see [`skene_common::RawCheckbox`].
    pub type RawCheckbox<C = ::skene_core::NoChild> = ::skene_common::RawCheckbox<$platform, C>;
    /// A slider; see [`skene_common::RawSlider`].
    pub type RawSlider<C = ::skene_core::NoChild> = ::skene_common::RawSlider<$platform, C>;
    /// A radio group; see [`skene_common::RawRadioGroup`].
    pub type RawRadioGroup<C = ::skene_core::NoChild> =
      ::skene_common::RawRadioGroup<$platform, C>;
    /// A radio inside a group; see [`skene_common::RawRadio`].
    pub type RawRadio<C = ::skene_core::NoChild> = ::skene_common::RawRadio<$platform, C>;
    /// A single-line text entry; see [`skene_common::RawTextField`].
    pub type RawTextField<C = ::skene_core::NoChild> =
      ::skene_common::RawTextField<$platform, C>;
    /// A multi-line text entry; see [`skene_common::RawTextEditor`].
    pub type RawTextEditor<C = ::skene_core::NoChild> =
      ::skene_common::RawTextEditor<$platform, C>;
    /// A listbox; see [`skene_common::RawListbox`].
    pub type RawListbox<C = ::skene_core::NoChild> = ::skene_common::RawListbox<$platform, C>;
    /// An option inside a listbox or a picker; see [`skene_common::RawOption`].
    pub type RawOption<C = ::skene_core::NoChild> = ::skene_common::RawOption<$platform, C>;
    /// A picker: a popup face, an overlay and a listbox; see
    /// [`skene_common::RawPicker`].
    pub type RawPicker<F = ::skene_core::NoChild> = ::skene_common::RawPicker<$platform, F>;
    /// A picker's face; see [`skene_common::Face`].
    pub type Face = ::skene_common::Face<$platform>;
    /// Content above the app; see [`skene_common::Overlay`].
    pub type Overlay<C = ::skene_core::NoChild> = ::skene_common::Overlay<$platform, C>;
    /// Content shown beside the view it describes; see [`skene_common::Tooltip`].
    pub type Tooltip<C = ::skene_core::NoChild> = ::skene_common::Tooltip<$platform, C>;
    /// Commands shown beside the pointer on a secondary press over the view it serves;
    /// see [`skene_common::ContextMenu`].
    pub type ContextMenu<C = ::skene_core::NoChild> = ::skene_common::ContextMenu<$platform, C>;
    /// Keyboard shortcuts over the view it serves; see [`skene_common::Keymap`].
    pub type Keymap<C = ::skene_core::NoChild> = ::skene_common::Keymap<$platform, C>;
    /// A menu; see [`skene_common::RawMenu`].
    pub type RawMenu<C = ::skene_core::NoChild> = ::skene_common::RawMenu<$platform, C>;
    /// One item within a menu; see [`skene_common::RawMenuItem`].
    pub type RawMenuItem<C = ::skene_core::NoChild> = ::skene_common::RawMenuItem<$platform, C>;
    /// A handle naming the node a view built; see [`skene_common::NodeHandle`].
    pub type NodeHandle = ::skene_common::NodeHandle<$platform>;
    /// A box drawn by a paint callback; see [`skene_common::Painted`].
    pub type Painted = ::skene_common::Painted<$platform>;
    /// A scrolling keyed list building only the slice in view; see
    /// [`skene_common::LazyColumn`].
    pub type LazyColumn = ::skene_common::LazyColumn<$platform>;
    /// A scrolling keyed row building only the slice in view; see
    /// [`skene_common::LazyRow`].
    pub type LazyRow = ::skene_common::LazyRow<$platform>;
    /// The closure building a view's child under the context the view provides; see
    /// [`skene_common::ChildBuilder`].
    pub type ChildBuilder = ::skene_common::ChildBuilder<$platform>;

    #[doc = concat!("A condition on the environment, in ", $unit, "; see [`skene_core::Media`].")]
    pub type Media = ::skene_core::Media<$scalar>;
    /// A condition a property varies under; see [`skene_core::Condition`].
    pub type Condition = ::skene_core::Condition<$scalar>;
    #[doc = concat!("A test of a container's size, in ", $unit, "; see [`skene_core::ContainerQuery`].")]
    pub type ContainerQuery = ::skene_core::ContainerQuery<$scalar>;
    /// A property's value: a leaf under conditions; see [`skene_core::Value`].
    pub type Value<T> = ::skene_core::Value<T, Condition>;
    /// A token's definition; see [`skene_core::Definition`].
    pub type Definition<T> = ::skene_core::Definition<T, $scalar>;
    /// A token as a palette holds it; see [`skene_core::Token`].
    pub type Token<T> = ::skene_core::Token<T, $scalar>;
    #[doc = concat!("Spacing on the four edges of a box, in ", $unit, ".")]
    pub type Inset<T = $scalar> = ::skene_core::Inset<T>;
    #[doc = concat!("A dimension, in ", $unit, " or as a fraction of the parent.")]
    pub type Dimension = ::skene_core::Dimension<$scalar>;
    #[doc = concat!("Per-axis spacing between children, in ", $unit, ".")]
    pub type Spacing = ::skene_core::Spacing<$scalar>;
    #[doc = concat!("A 2D size, in ", $unit, ".")]
    pub type Size = ::skene_core::Size<$scalar>;
    #[doc = concat!("A point, in ", $unit, ".")]
    pub type Point = ::skene_core::Point<$scalar>;
    #[doc = concat!("A positioned, sized rectangle, in ", $unit, ".")]
    pub type Rect = ::skene_core::Rect<$scalar>;

    /// Mounts a reactive subtree between anchors, rebuilt whenever a signal `build`
    /// reads changes; see [`skene_core::derived`].
    #[inline(always)]
    pub fn derived(build: impl Fn() -> ::skene_core::NodeId + 'static) -> ::skene_core::NodeId {
      ::skene_core::derived::<$platform>(build)
    }

    /// Reactively reconciles the content of the segment `start` begins from a keyed
    /// collection; see [`skene_core::reconcile`].
    #[inline(always)]
    pub fn reconcile<T, I, K>(
      start: ::skene_core::NodeId,
      items: impl Fn() -> I + 'static,
      key: impl Fn(&T) -> K + 'static,
      build: impl Fn(T) -> ::skene_core::NodeId + 'static,
    ) where
      I: IntoIterator<Item = T>,
      K: Eq + ::std::hash::Hash + 'static,
    {
      ::skene_core::reconcile::<$platform, T, I, K>(start, items, key, build);
    }

    /// A view that renders nothing.
    #[inline(always)]
    #[must_use]
    pub fn empty() -> impl View {
      ::skene_core::empty::<$platform>()
    }
  };
}
