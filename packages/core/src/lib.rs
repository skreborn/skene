//! The shared foundation.

mod boundary;
mod children;
mod condition;
mod events;
mod node;
mod props;
mod reconcile;
mod reorder;
mod runtime;
mod source;
mod style;
mod tree;
mod values;
mod view;

pub use boundary::Boundary;
pub use children::{Children, HasChildren, IntoChildren};
pub use condition::{Composable, ConditionShape};
pub use events::KeyEvent;
pub use node::NodeId;
pub use props::{BoxProp, BoxProps, BoxStyle, InputProps, RunDisplay, RunProp, RunProps, RunStyle};
pub use reconcile::{DynamicView, dynamic, empty, reconcile};
pub use reorder::Reorder;
pub use runtime::{reset_root_scope, root_scope};
pub use source::Source;
pub use style::{Animation, Condition, Easing, StyleProperty, Transition};
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
  Point,
  Rect,
  Role,
  Size,
};
pub use view::{AnyView, IntoView, View};
