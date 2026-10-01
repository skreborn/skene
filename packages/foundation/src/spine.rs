//! The spine: the scene, the platform seam, event dispatch, composition, and the
//! property, condition, expression and token model, naming nothing outside itself.

mod box_style;
mod children;
mod condition;
mod condition_tree;
mod control;
mod derived;
mod dispatch;
mod drag;
mod errors;
mod events;
mod expr;
mod extension;
mod fingerprint;
mod handler;
mod keyed;
mod leaf;
mod motion;
mod naming;
mod node;
mod node_map;
mod platform;
mod property;
mod props;
mod reorder;
mod resolve;
mod run_style;
mod runtime;
mod scalar;
mod scene;
mod segment;
mod shape;
pub mod task;
#[cfg(test)]
mod testing;
mod token;
pub mod typography;
mod values;
mod view;
pub mod window;
mod wraps;

pub use accesskit::TreeUpdate;
pub use box_style::{
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxSlot,
  BoxStyle,
  BoxValue,
  BoxVisitor,
  IntoBoxValue,
  box_fields,
};
pub use children::{Children, HasChild, HasChildren, IntoChildren, Seal};
pub use condition::{
  Condition,
  ContainerName,
  ContainerQuery,
  Environment,
  Media,
  MediaEnv,
  PointerPrecision,
  State,
};
pub use condition_tree::{ConditionShape, ConditionTree};
pub use control::{ControlProps, ControlState, NativeElement, Part, PickerOption};
pub use derived::{derived, empty, reconcile};
pub use dispatch::{
  dispatch_click,
  dispatch_click_observed,
  dispatch_context_menu,
  dispatch_double_click,
  dispatch_key_down,
  dispatch_key_up,
  dispatch_pointer_cancel,
  dispatch_pointer_down,
  dispatch_pointer_move,
  dispatch_pointer_up,
  dispatch_wheel,
  notify_change,
  notify_dismiss,
  notify_focus_moved,
  notify_pointer_moved,
  notify_scroll,
  notify_selection_change,
  notify_toggle,
};
pub use drag::{
  DragData,
  DragSource,
  DropHandler,
  DropPhase,
  drop_drag,
  end_drag,
  move_drag,
  start_drag,
};
pub use errors::Errors;
pub use events::{
  Bubbling,
  ClickEvent,
  ClickHandler,
  KeyDefault,
  KeyEvent,
  KeyHandler,
  Modifiers,
  PointerEvent,
  PointerHandler,
  Prevent,
  Prevented,
  ScrollEvent,
  Shortcut,
  WheelEvent,
  WheelHandler,
  WheelUnit,
};
pub use expr::{Expr, ExprEnv, ExprNode, ExprOutput, ExprValue, Length, Number, Quantity};
pub use extension::{NoProperty, NoValue, StyleExtension};
pub use fingerprint::Fingerprint;
pub use handler::{
  ChangeHandler,
  Handler,
  PlainHandler,
  Registered,
  ScrollHandler,
  SelectionHandler,
  ToggleHandler,
};
pub use keyboard_types::{Code, Key, NamedKey};
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
pub use naming::NameFacts;
pub use node::NodeId;
pub use node_map::NodeMap;
pub use platform::{Observation, Platform, Timer, Timers};
pub use property::{Property, PropertyKey, PropertyName};
pub use props::{
  BoxKeyframe,
  BoxProps,
  ContainerSizes,
  DeclaredBoxStyle,
  DeclaredRunStyle,
  EventKind,
  MediaSignals,
  MediaWriters,
  OverlayProps,
  RunKeyframe,
  RunProps,
  StaticBoxStyle,
  StaticRunStyle,
};
pub use reorder::Reorder;
pub use resolve::{
  Focus,
  Interaction,
  ResolutionContext,
  ResolutionSource,
  Resolved,
  Resolver,
  SlotLeaf,
  SlotValue,
  TokenValue,
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
  run_fields,
};
pub use runtime::{end_turn, hold_flushes, reset_thread};
pub use scalar::Scalar;
pub use scene::{Annotated, Annotation, NodeKind, NodeRecord, Scene};
pub use segment::{Piece, Segment};
pub use shape::{Declared, Keyframe, Plain, Shape, Static};
pub use task::{Boundary, count_pending};
pub use token::{Definitions, IntoTokenValue, Token, TokenName, TokenRef, TokenSet, Tokenable};
pub use ui_events::pointer::{
  PersistentDeviceId,
  PointerButton,
  PointerButtons,
  PointerId,
  PointerInfo,
  PointerType,
};
pub use values::{
  AccessProp,
  Alignment,
  Arrangement,
  AspectRatio,
  Availability,
  Axes,
  Axis,
  CheckState,
  ClipboardAction,
  Color,
  ColorScheme,
  Dimension,
  DragAxes,
  Edge,
  Flexibility,
  FocusOrder,
  GridArea,
  GridAreas,
  GridFlow,
  GridName,
  GridPlacement,
  GridTemplate,
  GridTrack,
  Inset,
  ItemAlignment,
  Layout,
  Line,
  MouseCursor,
  Overflow,
  Placement,
  Point,
  PopupKind,
  Rect,
  Role,
  RunPlacement,
  Size,
  Spacing,
  TemplateEntry,
  TextDecoration,
  TextDecorationStyle,
  TextSelection,
  TrackLimit,
  Wrap,
};
pub use view::{
  AnyKind,
  AnyView,
  FlexChild,
  GridChild,
  IntoChildOf,
  NoChild,
  PlainChild,
  StackChild,
  View,
};
pub use wraps::Wraps;
