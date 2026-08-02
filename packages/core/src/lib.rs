//! The shared foundation.

mod boundary;
mod children;
mod condition;
mod errors;
mod events;
mod node;
mod props;
mod reconcile;
mod reorder;
mod runtime;
mod source;
mod style;
mod suspense;
mod task;
mod tree;
mod values;
mod view;
mod virtualize;

pub use boundary::Boundary;
pub use children::{Children, HasChildren, IntoChildren};
pub use condition::{Composable, ConditionShape};
pub use errors::{ErrorBoundary, Errors};
pub use events::{KeyEvent, Modifiers, PointerButton, PointerEvent, ScrollEvent};
pub use node::NodeId;
pub use props::{
  BoxProp,
  BoxProps,
  BoxStyle,
  ButtonProps,
  CheckboxProps,
  ControlProps,
  InputProps,
  OverlayProps,
  RadioProps,
  RunDisplay,
  RunProp,
  RunProps,
  RunStyle,
  SelectOption,
  SelectProps,
  SliderProps,
  TextareaProps,
};
pub use reconcile::{DynamicView, dynamic, empty, reconcile};
pub use reorder::Reorder;
pub use runtime::{reset_root_scope, root_scope};
pub use source::Source;
pub use style::{Animation, Condition, Easing, StyleProperty, Transition};
pub use suspense::Suspense;
pub use task::scope_bound;
pub use tree::TreeOps;
pub use values::{
  Axis,
  Border,
  Color,
  CrossAxisAlignment,
  Inset,
  Layout,
  MainAxisAlignment,
  MouseCursor,
  Overflow,
  Placement,
  Point,
  Rect,
  Role,
  Size,
};
pub use view::{AnyView, IntoView, View};
pub use virtualize::{VirtualWindow, virtual_window};
