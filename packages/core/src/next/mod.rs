//! The scene, the driver seam, event dispatch, composition, and the property,
//! condition, expression and token model.
//!
//! Transitional home of what replaces the crate's flat exports ([`BoxStyle`](crate::BoxStyle),
//! [`Condition`](crate::Condition), [`Adaptive`](crate::Adaptive), [`Token`](crate::Token),
//! [`TreeOps`](crate::TreeOps), [`View`](crate::View), [`Children`](crate::Children) and
//! their kin): both stay until every facade builds on this module, after which it is
//! flattened into the crate root under these same names.
//!
//! Core owns the retained tree: a [`Scene`] of [`NodeRecord`]s addressed by generational
//! [`NodeId`]s, every mutation mirrored into the sink of the one [`Driver`] a facade
//! names. Dispatch ([`dispatch_click`] and its kin) climbs that tree once for every
//! driver. A [`View`] builds into a node; reactive content ([`derived`], [`reconcile`],
//! [`Show`]) mounts between a segment's anchors, and a container's [`Children`] is a
//! list of such segments.
//!
//! A property's value is a [`Value`]: a [`Leaf`]—fixed, a token reference, an expression,
//! or derived from signals—under conditions ([`Condition`]) that pick a variant. A
//! [`Token`]'s [`Definition`] is the same shape without the derived leaf and with only
//! [`Media`] conditions, so a palette is plain data. A style ([`BoxStyle`], [`RunStyle`])
//! holds one field per property in a [`Shape`]: what a builder collects, what a driver
//! receives, or plain values. Every facade fixes the [`Scalar`] its lengths are measured
//! in and the [`StyleExtension`] holding the properties only its medium has.

mod border;
mod box_style;
mod children;
mod condition;
mod control;
mod derived;
mod dispatch;
mod driver;
mod expr;
mod extension;
mod leaf;
mod node;
mod property;
mod props;
mod run_style;
mod scalar;
mod scene;
mod shape;
mod show;
#[cfg(test)]
mod testing;
mod token;
mod view;

pub use border::Border;
pub use box_style::{
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxSlot,
  BoxStyle,
  BoxValue,
  BoxVisitor,
  BoxVisitorMut,
  IntoBoxValue,
  box_fields,
};
pub use children::{Children, HasChildren, IntoChildren, Seal};
pub use condition::{Condition, ContainerName, ContainerQuery, Media, MediaEnv, State};
pub use control::{ControlKind, ControlProps, ControlState, Part};
pub use derived::{derived, empty, reconcile};
pub use dispatch::{
  dispatch_click,
  dispatch_key_down,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  notify_blur,
  notify_change,
  notify_focus,
  notify_pointer_enter,
  notify_pointer_leave,
  notify_scroll,
  notify_toggle,
};
pub use driver::Driver;
pub use expr::{Expr, ExprEnv, ExprNode, ExprOutput, ExprValue, Length, Number, Quantity};
pub use extension::{NoProperty, NoValue, StyleExtension};
pub use leaf::{
  Conditional,
  Definition,
  DerivedSlot,
  DriverLeaf,
  Leaf,
  Slot,
  StaticLeaf,
  StaticValue,
  Value,
  When,
};
pub use node::NodeId;
pub use property::{Property, PropertyKey, PropertyName};
pub use props::{
  BoxProps,
  ContainerSizes,
  DeclaredBoxStyle,
  DeclaredRunStyle,
  EventKind,
  MediaSignals,
  RunProps,
  StaticBoxStyle,
  StaticRunStyle,
};
pub use run_style::{
  IntoRunValue,
  RunDynamics,
  RunField,
  RunFieldVisitor,
  RunSlot,
  RunStyle,
  RunValue,
  RunVisitor,
  RunVisitorMut,
  run_fields,
};
pub use scalar::Scalar;
pub use scene::{
  Annotated,
  Annotation,
  ChangeHandler,
  HandlerSet,
  NodeKind,
  NodeRecord,
  PlainHandler,
  Scene,
  ScrollHandler,
  ToggleHandler,
};
pub use shape::{Declared, Plain, Shape, Static};
pub use show::Show;
pub use token::{Definitions, IntoTokenValue, Themed, Token, TokenName, TokenRef, TokenSet};
pub use view::{
  AnyKind,
  AnyView,
  FlexChild,
  GridChild,
  IntoChildOf,
  IntoView,
  NoChild,
  PlainChild,
  StackChild,
  View,
};
