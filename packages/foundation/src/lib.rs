//! # skene-foundation
//!
//! What every platform shares: the spine, and every built-in view once over it.
//!
//! The spine owns the retained tree: a [`Scene`] of [`NodeRecord`]s addressed by generational
//! [`NodeId`]s, every mutation mirrored into the sink of the one [`Realization`] a facade
//! names. Dispatch ([`dispatch_click`] and its kin) climbs that tree once for every
//! platform. A [`View`] builds into a node; reactive content ([`derived`], [`reconcile`],
//! [`Visibility`]) mounts between a segment's anchors, and a container's [`Children`] is a
//! list of such segments.
//!
//! A property's value is a [`Value`]: a [`Leaf`]—fixed, a token reference, an expression,
//! or derived from signals—under conditions ([`Condition`]) that pick a variant. A
//! [`Token`]'s [`Definition`] is the same shape without the derived leaf and with only
//! [`Media`] conditions, so a palette is plain data. A style ([`BoxStyle`], [`RunStyle`])
//! holds one field per property in a [`Shape`]: what a builder collects, what a node's
//! record is built from, or plain values. Every facade fixes the [`Scalar`] its lengths are
//! measured in and the [`StyleExtension`] holding the properties only its medium has, which its
//! medium's module here defines. A style's [`Fingerprint`] keys what a
//! platform compiled from it. Motion ([`Transition`], [`Animation`]) is data every
//! platform compiles or runs, and the universal values ([`Color`], the geometry, the
//! layout vocabulary) are what every medium's properties are made of.
//!
//! Every built-in view once, generic over the [`Realization`] a facade names. A facade
//! aliases them (`type Container = skene_foundation::Container<Active>`); a component
//! library writes against them exactly as a facade does. What every platform of one
//! medium shares above the spine is a module here: [`terminal`] for the terminal and
//! [`web`] for the web; [`routing`] is the router every facade binds. [`text`] is a text as an
//! engine receives it, every property resolved, and the contract a realization's text engine
//! lays it out under, with what is written once above it, the editing core every drawn
//! entry shares ([`text::editing`]) included. What a node shows crosses as one typed
//! [`Content`] a view writes through its key in [`contents`], whole: an entry's change
//! crosses as one [`Edit`], its text and its selection together, [`report`]ed to the
//! [`EditController`] of the content and answered by a [`Verdict`], a range's value with its
//! bounds as one [`Range`] and a choice's chosen key with its options as one [`Choices`], each
//! held whole by a [`Controller`] that repairs what the truth refuses by writing it, each
//! controller and key reaching the realization through the plain functions of a [`ContentOps`]
//! filled once from it; a drawn entry reports its text to assistive technology as an
//! [`AccessText`], read from its editor.
//!
//! A view's build folds the annotation a wrapper left pending into its declared style,
//! splits the style into the static part its record is built from (compiled at
//! [`Keep::Everything`] and handed over by [`Realization::apply_style`]) and the derived
//! slots render effects write through [`Realization::apply_slot`], creates its node,
//! registers it in the [`Scene`], binds each slot, registers its handlers, mounts its
//! children and returns its id. [`LayoutBox`] is that
//! build for a box; [`Container`], [`Row`], [`Column`], [`Grid`] and [`Stack`] are thin
//! vocabularies over it, and a third party's layout view is built from it the same way.
//! [`Flexible`], [`Pinned`] and [`GridCell`] annotate the box built under them. A run
//! ([`Text`], [`Span`], [`RichText`]) inherits through the [`typography`] tokens.
//! [`Visibility`], [`OneOf`], [`Suspense`] and [`ErrorBoundary`] mount between anchors and
//! keep the branches they hide. [`Theme`] defines a token set beneath it, and
//! [`ThemedIn`] reads the set in force. [`Control`] is the one control builder, over a
//! [`Behavior`] saying what the interaction is, acting through a [`ControlHandle`] whose
//! [`ControlOps`] reach the build's realization as plain functions, so a behavior names
//! none; [`Activate`], [`Toggle`], [`Slider`],
//! [`RadioGroup`] with [`Radio`], [`Popup`], [`Listbox`] with [`ListboxOption`], and
//! [`TextEntry`] ship, and the `Basic*` aliases name the control over each; a choice's
//! value is the app's own type, a [`Choice`], which the cores hold erased ([`AnyChoice`],
//! [`AnyChosen`]) and the typed views are [`Thin`] over. [`Overlay`]
//! is content above the app, opened and closed by the app and placed against the view
//! a [`NodeHandle`] is bound to, which focus also moves to and whose measured size it
//! reports; [`BasicPicker`] composes a popup face, an overlay and a listbox. [`LazyColumn`] and
//! [`LazyRow`] scroll a keyed list building only the slice in view, its items of a fixed or an
//! estimated and measured [`ItemExtent`]. [`CornerRadius`] is what a decorated box asks of a
//! medium's style extension to round its corners, and [`TypeMetrics`] what a run asks of it to size
//! its type; both are silently declined by a medium without them. [`LowLevel`] registers a sink a
//! low-level view made against a platform's escape trait as a node, so it composes like a built-in,
//! and adopts a compositor platform's own view through the native door, a value it holds
//! controlled by a [`Controller`] through the [`ControlSink`] registered on its node.

mod apply;
mod container;
mod context_menu;
mod control;
mod corner_radius;
mod enhance;
mod error_boundary;
mod facade;
mod flex;
mod flexible;
mod grid;
mod grid_cell;
mod keymap;
mod layout_box;
mod lazy_list;
mod link;
mod macros;
pub mod media;
mod node_handle;
mod one_of;
mod overlay;
mod painted;
mod painting;
mod picker;
mod pinned;
pub mod realize;
pub mod routing;
mod run;
mod span;
mod spine;
mod stack;
mod surface;
mod suspense;
pub mod terminal;
pub mod text;
mod theme;
mod thin;
mod tooltip;
mod type_metrics;
mod visibility;
pub mod web;

extern crate self as skene_foundation;

pub use skene_macros::{View, Wraps};
pub use skene_reactive as reactive;

pub use self::spine::*;
pub use crate::{
  apply::Apply,
  container::{Container, ContainerBase},
  context_menu::{
    ContextMenu,
    ContextMenuBase,
    ContextMenuWithCornerRadius,
    MenuEntry,
    MenuItemContent,
  },
  control::{
    Activate,
    AnyChoice,
    AnyChosen,
    BasicButton,
    BasicCheckbox,
    BasicListboxBase,
    BasicListboxOption,
    BasicMenu,
    BasicMenuItem,
    BasicRadio,
    BasicRadioGroupBase,
    BasicSlider,
    BasicTextEditor,
    BasicTextField,
    Behavior,
    Choice,
    Chosen,
    Collection,
    CollectionContext,
    CollectionPolicy,
    Control,
    ControlBase,
    ControlHandle,
    ControlOps,
    Controlled,
    ControlledChoice,
    Label,
    LabelPlacement,
    Listbox,
    ListboxContext,
    ListboxOption,
    ListboxPolicy,
    Member,
    Menu,
    MenuContext,
    MenuOption,
    MenuPolicy,
    NativePicker,
    PointerPhase,
    Popup,
    Radio,
    RadioGroup,
    RadioGroupContext,
    RadioPolicy,
    Ring,
    Roster,
    Slider,
    SliderRange,
    TYPE_AHEAD_RESET,
    TextEntry,
    Toggle,
    TypeAhead,
  },
  corner_radius::CornerRadius,
  enhance::{Enhance, EnhanceBase},
  error_boundary::{ErrorBoundary, ErrorBoundaryBase},
  flex::{Column, ColumnBase, FlexBase, Row, RowBase},
  flexible::{Flexible, FlexibleBase},
  grid::{Grid, GridBase},
  grid_cell::{GridCell, GridCellBase},
  keymap::{Keymap, KeymapBase},
  layout_box::LayoutBox,
  lazy_list::{
    ItemExtent,
    ItemWindow,
    LazyColumn,
    LazyColumnBase,
    LazyRow,
    LazyRowBase,
    item_window,
    item_window_over,
  },
  link::{Link, LinkBase},
  node_handle::NodeHandle,
  one_of::{OneOf, OneOfBase},
  overlay::{Overlay, OverlayBase},
  painted::{Painted, PaintedBase},
  painting::{Canvas, IntoBrush, PaintType, Painter, Painting, Renderer, kurbo, paint, peniko},
  picker::{BasicPicker, BasicPickerBase, BasicPickerCore},
  pinned::{Pinned, PinnedBase},
  realize::{ChildBuilder, LowLevel},
  run::Run,
  span::{RichText, RichTextBase, Span, SpanBase},
  stack::{Stack, StackBase},
  surface::{
    DecorationBase,
    DecorationWithCornerRadius,
    HasBoxStyle,
    HasLayoutBox,
    HasRun,
    MotionBase,
    RunBase,
    RunWithTypeMetrics,
  },
  suspense::{Suspense, SuspenseBase},
  text::{Text, TextBase},
  theme::{Theme, ThemeBase, ThemedIn},
  thin::Thin,
  tooltip::{TOOLTIP_DELAY, TOOLTIP_GRACE, Tooltip, TooltipBase},
  type_metrics::TypeMetrics,
  visibility::{Visibility, VisibilityBase},
};
