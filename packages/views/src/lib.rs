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
//! [`Roving`] with [`RadioMember`], and [`TextEntry`] ship, and the `Raw*` aliases name
//! the control over each. [`CornerRadius`] is what a decorated box asks of a medium's
//! style extension to round its corners, silently declined by a medium without radii.

mod container;
mod control;
mod corner_radius;
mod error_boundary;
mod flex;
mod flexible;
mod grid;
mod grid_cell;
mod layout_box;
mod macros;
mod pinned;
pub mod realize;
mod run;
mod span;
mod stack;
mod suspense;
mod switch;
mod text;
mod theme;
pub mod typography;

pub use skene_core::{
  Boundary,
  Errors,
  next::{
    AnyKind,
    AnyView,
    ControlKind,
    ControlState,
    Driver,
    HasChildren,
    IntoChildOf,
    IntoChildren,
    IntoView,
    NoChild,
    Part,
    Scene,
    Show,
    View,
  },
};

pub use crate::{
  container::Container,
  control::{
    Activate,
    Behavior,
    Control,
    ControlChild,
    ControlHandle,
    Label,
    PointerPhase,
    RadioMember,
    Range,
    RawButton,
    RawCheckbox,
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
  grid::Grid,
  grid_cell::GridCell,
  layout_box::LayoutBox,
  pinned::Pinned,
  realize::ChildBuilder,
  span::{RichText, Span},
  stack::Stack,
  suspense::Suspense,
  switch::Switch,
  text::Text,
  theme::{Theme, ThemedIn},
};
