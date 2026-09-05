//! # Built-in views
//!
//! Every built-in view once, generic over the [`Driver`] a facade names. A facade
//! aliases them (`type Container = skene_views::Container<ActiveDriver>`); a component
//! library writes against them exactly as a facade does.
//!
//! A view's build folds the annotation a wrapper left pending into its declared style,
//! splits the style into the static part a driver compiles and the derived slots render
//! effects write, creates its node, registers it in the [`Scene`], binds each slot,
//! registers its handlers, mounts its children and returns its id. [`LayoutBox`] is that
//! build for a box; [`Container`], [`Row`], [`Column`], [`Grid`] and [`Stack`] are thin
//! vocabularies over it, and a third party's layout view is built from it the same way.
//! [`Flexible`], [`Pinned`] and [`GridCell`] annotate the box built under them. A run
//! ([`Text`], [`Span`], [`RichText`]) inherits through the [`typography`] tokens.
//! [`Show`], [`Switch`], [`Suspense`] and [`ErrorBoundary`] mount between anchors and
//! keep the branches they hide. [`Theme`] defines a token set beneath it, and
//! [`ThemedIn`] reads the set in force. [`Control`] is the one control builder, over a
//! [`Behavior`] saying what the interaction is; [`Activate`], [`Toggle`], [`Range`],
//! [`Roving`] with [`RadioMember`], [`Popup`], [`Listbox`] with [`ListItem`], and
//! [`TextEntry`] ship, and the `Raw*` aliases name the control over each. [`Overlay`]
//! is content above the app, opened and closed by the app and placed against an
//! [`Anchor`]; [`RawPicker`] composes a popup face, an overlay and a listbox; a
//! [`FocusHandle`] moves focus to the view it is bound to. [`LazyColumn`] scrolls a
//! keyed list building only the slice in view. [`CornerRadius`] is what a decorated
//! box asks of a medium's style extension to round its corners, and [`TypeMetrics`]
//! what a run asks of it to size its type; both are silently declined by a medium
//! without them. [`LowLevel`] registers a sink a low-level view made against a
//! driver's escape trait as a node, so it composes like a built-in.

mod anchor;
mod container;
mod control;
mod corner_radius;
mod error_boundary;
mod flex;
mod flexible;
mod focus;
mod grid;
mod grid_cell;
mod layout_box;
mod lazy_column;
mod macros;
mod overlay;
mod picker;
mod pinned;
pub mod realize;
mod run;
mod span;
mod stack;
mod suspense;
mod switch;
mod text;
mod theme;
mod type_metrics;
pub mod typography;

#[doc(hidden)]
pub use skene_core as __core;
pub use skene_core::{
  AnyKind,
  AnyView,
  Boundary,
  ControlKind,
  ControlState,
  Driver,
  Errors,
  HasChildren,
  IntoChildOf,
  IntoChildren,
  IntoView,
  NoChild,
  NodeKind,
  Part,
  Scene,
  Show,
  View,
};

pub use crate::{
  anchor::Anchor,
  container::Container,
  control::{
    Activate,
    Behavior,
    Control,
    ControlChild,
    ControlHandle,
    Label,
    ListItem,
    Listbox,
    ListboxGroup,
    PointerPhase,
    Popup,
    RadioMember,
    Range,
    RawButton,
    RawCheckbox,
    RawListbox,
    RawOption,
    RawRadio,
    RawRadioGroup,
    RawSlider,
    RawTextEditor,
    RawTextField,
    Roster,
    Roving,
    RovingGroup,
    TextEntry,
    Toggle,
  },
  corner_radius::CornerRadius,
  error_boundary::ErrorBoundary,
  flex::{Column, Row},
  flexible::Flexible,
  focus::FocusHandle,
  grid::Grid,
  grid_cell::GridCell,
  layout_box::LayoutBox,
  lazy_column::LazyColumn,
  overlay::Overlay,
  picker::{Face, RawPicker},
  pinned::Pinned,
  realize::{ChildBuilder, LowLevel},
  span::{RichText, Span},
  stack::Stack,
  suspense::Suspense,
  switch::Switch,
  text::Text,
  theme::{Theme, ThemedIn},
  type_metrics::TypeMetrics,
};
