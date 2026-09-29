//! # skene-common
//!
//! Every built-in view once, generic over the [`Platform`] a facade names. A facade
//! aliases them (`type Container = skene_common::Container<ActivePlatform>`); a component
//! library writes against them exactly as a facade does. What every platform of one
//! medium shares above the spine is a module here: [`gui`] for pixels and [`tui`] for the
//! terminal; [`routing`] is the router every facade binds.
//!
//! A view's build folds the annotation a wrapper left pending into its declared style,
//! splits the style into the static part a platform compiles and the derived slots render
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
//! [`FocusHandle`] moves focus to the view it is bound to, [`SizeHandle`] reports its
//! measured size. [`LazyColumn`] and [`LazyRow`] scroll a keyed list building only the
//! slice in view, its items of a fixed or an estimated and measured [`ItemExtent`].
//! [`CornerRadius`] is what a decorated box asks of a medium's style extension to round its
//! corners, and [`TypeMetrics`] what a run asks of it to size its type; both are silently declined
//! by a medium without them. [`LowLevel`] registers a sink a low-level view made against a
//! platform's escape trait as a node, so it composes like a built-in.

mod anchor;
mod container;
mod context_menu;
mod control;
mod corner_radius;
mod enhance;
mod error_boundary;
mod flex;
mod flexible;
mod focus;
mod grid;
mod grid_cell;
pub mod gui;
mod keymap;
mod layout_box;
mod lazy_list;
mod macros;
pub mod media;
mod overlay;
mod painted;
mod painting;
mod picker;
mod pinned;
pub mod realize;
pub mod routing;
mod run;
mod size;
mod span;
mod stack;
mod suspense;
mod switch;
mod text;
mod theme;
mod tooltip;
pub mod tui;
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
  Errors,
  HasChildren,
  IntoChildOf,
  IntoChildren,
  IntoView,
  Keyed,
  NoChild,
  NodeKind,
  Part,
  Platform,
  Scene,
  Show,
  View,
};

pub use crate::{
  anchor::Anchor,
  container::Container,
  context_menu::{ContextMenu, MenuItem, MenuItemContent},
  control::{
    Activate,
    Behavior,
    Control,
    ControlChild,
    ControlHandle,
    Controlled,
    Label,
    LabelPlacement,
    ListItem,
    Listbox,
    ListboxGroup,
    Menu,
    MenuGroup,
    MenuOption,
    NativePicker,
    PointerPhase,
    Popup,
    RadioMember,
    Range,
    RawButton,
    RawCheckbox,
    RawListbox,
    RawMenu,
    RawMenuItem,
    RawOption,
    RawRadio,
    RawRadioGroup,
    RawSlider,
    RawTextEditor,
    RawTextField,
    Roster,
    Roving,
    RovingGroup,
    TYPE_AHEAD_RESET,
    TextEntry,
    Toggle,
    TypeAhead,
  },
  corner_radius::CornerRadius,
  enhance::Enhance,
  error_boundary::ErrorBoundary,
  flex::{Column, Row},
  flexible::Flexible,
  focus::FocusHandle,
  grid::Grid,
  grid_cell::GridCell,
  keymap::Keymap,
  layout_box::LayoutBox,
  lazy_list::{ItemExtent, LazyColumn, LazyRow},
  overlay::Overlay,
  painted::Painted,
  painting::{Canvas, IntoBrush, PaintType, Painter, Painting, Renderer, kurbo, paint, peniko},
  picker::{Face, RawPicker},
  pinned::Pinned,
  realize::{ChildBuilder, LowLevel},
  size::SizeHandle,
  span::{RichText, Span},
  stack::Stack,
  suspense::Suspense,
  switch::Switch,
  text::Text,
  theme::{Theme, ThemedIn},
  tooltip::{TOOLTIP_DELAY, TOOLTIP_GRACE, Tooltip},
  type_metrics::TypeMetrics,
};
