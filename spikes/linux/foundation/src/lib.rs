//! # skene-foundation
//!
//! The mechanisms Skene's facades share, naming no platform and no toolkit: `skene-gui` (the
//! pixel platforms) stands on it, and so would its sibling `skene-tui`.
//!
//! - [`reactive`]: signals, memos and effects, owned by scopes ([`Owner`]).
//! - [`scene`]: a generational arena of nodes with parents and ordered children ([`Scene`]).
//! - [`keyed`]: a keyed list's items, each built once under an owner of its own ([`Keyed`]).
//! - [`value`]: a property's value ([`Value`]): fixed, a token, or derived from signals, and
//!   replaced under conditions (`16.when(Media::MinWidth(900), 24)`); and the types the
//!   properties hold.
//! - [`condition`]: what a value varies under ([`Media`], [`State`]).
//! - [`token`]: design tokens, named inherited slots whose definitions carry their own
//!   variants (dark mode among them), and the scope chain they resolve through ([`Tokens`]).
//! - [`environment`]: the facts conditions are answered against ([`Environment`]), which a
//!   realization reports ([`environment::report`]).
//! - [`layout`]: the layout engine (taffy), one small tree per container over a toolkit's own
//!   measuring ([`layout::ContainerLayout`] over [`layout::LayoutHost`]).
//! - [`behavior`]: what a control's interaction is ([`behavior::Behavior`]): the events it acts
//!   on and the verdict it gives each, what it tells a reader, and the state its parts are
//!   styled under; the key tables every platform that draws its controls shares; and the
//!   handle a behavior acts through ([`behavior::ControlHandle`]).
//! - [`system_colors`]: the platform's colors the built-in controls' defaults name
//!   ([`SystemColors`]), defined by the realization at the root.
//! - [`seam`]: what crosses between the facade's shared views and a platform's realization:
//!   the contract every realization implements ([`seam::Realization`]) and the shared half
//!   that calls it (the tree, the record and its resolver, the controlled contract, the
//!   accessibility merge, the testing driver's contract).
//!
//! Lengths are plain numbers of pixels: `16`, `-4` (`i32`) or `16.5` (`f32`).

pub mod behavior;
pub mod color;
pub mod condition;
pub mod environment;
pub mod keyed;
pub mod layout;
pub mod reactive;
pub mod scene;
pub mod seam;
pub mod system_colors;
pub mod text;
pub mod token;
pub mod value;

pub use crate::{
  color::Color,
  condition::{Condition, Media, State, StateSet},
  environment::{Environment, Scheme},
  keyed::Keyed,
  reactive::{Effect, Memo, Owner, Signal, batch, untrack},
  scene::{NodeId, Scene},
  system_colors::SystemColors,
  token::{
    Definition, DefinitionLeaf, Definitions, IntoDefinition, Token, TokenRef, TokenSet, TokenValue,
    Tokenable, Tokens,
  },
  value::{
    Alignment, Arrangement, AxisAlignment, Border, BorderStyle, Corners, Curve, Dimension, Edges,
    Flexibility, Flow, FontFamily, FontStyle, FontWeight, GridLine, GridPlacement, GridTrack,
    Inset, IntoUnit, Leaf, Offset, Property, Ring, Shadow, Shadows, Spacing, TextAlignment,
    TextDecoration, TextDecorationStyle, Tracks, Value, When,
  },
};
