//! The spine: the scene, the platform seam, event dispatch, composition, and the
//! property, condition, expression and token model.
//!
//! Core owns the retained tree: a [`Scene`] of [`NodeRecord`]s addressed by generational
//! [`NodeId`]s, every mutation mirrored into the sink of the one [`Platform`] a facade
//! names. Dispatch ([`dispatch_click`] and its kin) climbs that tree once for every
//! platform. A [`View`] builds into a node; reactive content ([`derived`], [`reconcile`],
//! [`Show`]) mounts between a segment's anchors, and a container's [`Children`] is a
//! list of such segments.
//!
//! A property's value is a [`Value`]: a [`Leaf`]—fixed, a token reference, an expression,
//! or derived from signals—under conditions ([`Condition`]) that pick a variant. A
//! [`Token`]'s [`Definition`] is the same shape without the derived leaf and with only
//! [`Media`] conditions, so a palette is plain data. A style ([`BoxStyle`], [`RunStyle`])
//! holds one field per property in a [`Shape`]: what a builder collects, what a platform
//! receives, or plain values. Every facade fixes the [`Scalar`] its lengths are measured
//! in and the [`StyleExtension`] holding the properties only its medium has, which its
//! medium's core crate defines. A style's [`Fingerprint`] keys what a platform compiled
//! from it. Motion ([`Transition`], [`Animation`]) is data every
//! platform compiles or runs, and the universal values ([`Color`], the geometry, the
//! layout vocabulary) are what every medium's properties are made of.

mod border;
mod boundary;
mod box_style;
mod children;
mod composable;
mod condition;
mod control;
mod derived;
mod dispatch;
mod errors;
mod events;
pub mod executor;
mod expr;
mod extension;
mod fingerprint;
mod keyed;
mod leaf;
mod motion;
mod node;
mod platform;
mod property;
mod props;
mod reorder;
mod resolve;
mod run_style;
mod runtime;
mod scalar;
mod scene;
mod shape;
mod show;
mod task;
#[cfg(test)]
mod testing;
mod token;
mod values;
mod view;
mod window;

pub use border::Border;
pub use boundary::Boundary;
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
pub use composable::{Composable, ConditionShape};
pub use condition::{Condition, ContainerName, ContainerQuery, Media, MediaEnv, State};
pub use control::{ControlKind, ControlProps, ControlState, Part, Ring, SliderRange};
pub use derived::{derived, empty, reconcile};
pub use dispatch::{
  dispatch_click,
  dispatch_click_observed,
  dispatch_context_menu,
  dispatch_key_down,
  dispatch_key_up,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  notify_blur,
  notify_change,
  notify_dismiss,
  notify_focus,
  notify_pointer_enter,
  notify_pointer_leave,
  notify_scroll,
  notify_selection_change,
  notify_toggle,
};
pub use errors::Errors;
pub use events::{
  Bubbling,
  ClickHandler,
  KeyEvent,
  KeyHandler,
  Modifiers,
  PointerButton,
  PointerEvent,
  PointerHandler,
  Prevent,
  Prevented,
  ScrollEvent,
  Shortcut,
};
pub use expr::{Expr, ExprEnv, ExprNode, ExprOutput, ExprValue, Length, Number, Quantity};
pub use extension::{NoProperty, NoValue, StyleExtension};
pub use fingerprint::Fingerprint;
pub use keyed::Keyed;
pub use leaf::{
  Conditional,
  Definition,
  DerivedSlot,
  Leaf,
  PlatformLeaf,
  Slot,
  StaticLeaf,
  StaticValue,
  Value,
  When,
};
pub use motion::{Animation, Easing, StyleProperty, Transition, interpolate};
pub use node::NodeId;
pub use platform::{Platform, Timer};
pub use property::{Property, PropertyKey, PropertyName};
pub use props::{
  BoxKeyframe,
  BoxProps,
  ContainerSizes,
  DeclaredBoxStyle,
  DeclaredRunStyle,
  EventKind,
  MediaSignals,
  OverlayProps,
  RunKeyframe,
  RunProps,
  StaticBoxStyle,
  StaticRunStyle,
};
pub use reorder::Reorder;
pub use resolve::{Focus, Interaction, Resolved, Resolver, SlotLeaf, SlotValue, TokenValue};
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
pub use runtime::{reset_root_owner, root_owner};
pub use scalar::Scalar;
pub use scene::{
  Annotated,
  Annotation,
  ChangeHandler,
  Handler,
  HandlerSet,
  NodeKind,
  NodeRecord,
  PlainHandler,
  Scene,
  ScrollHandler,
  SelectionHandler,
  ToggleHandler,
};
pub use shape::{Declared, Keyframe, Plain, Shape, Static};
pub use show::Show;
pub use task::scope_bound;
pub use token::{Definitions, IntoTokenValue, Themed, Token, TokenName, TokenRef, TokenSet};
pub use values::{
  AccessProp,
  Alignment,
  Arrangement,
  Availability,
  Axis,
  ClipboardAction,
  Color,
  ColorScheme,
  Dimension,
  FlexDirection,
  Flexibility,
  FocusOrder,
  GridFlow,
  GridPlacement,
  GridTrack,
  Inset,
  ItemAlignment,
  Layout,
  Line,
  LinkAnnotation,
  LinkPolicy,
  MouseCursor,
  Overflow,
  Placement,
  Point,
  PopupKind,
  Rect,
  Role,
  RunDisplay,
  Size,
  Spacing,
  TextDecoration,
  TextDecorationStyle,
  TextSelection,
  Toggle,
  TrackLimit,
  Wrap,
};
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
pub use window::{ItemWindow, item_window, item_window_over};
