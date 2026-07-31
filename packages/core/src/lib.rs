//! The shared foundation.

mod boundary;
mod children;
mod node;
mod props;
mod reorder;
mod runtime;
mod source;
mod style;
mod values;
mod view;

pub use boundary::Boundary;
pub use children::IntoChildren;
pub use node::NodeId;
pub use props::{BoxProp, BoxProps, BoxStyle, RunDisplay, RunProp, RunProps, RunStyle};
pub use reorder::Reorder;
pub use runtime::{reset_root_scope, root_scope};
pub use source::Source;
pub use style::{Animation, Condition, Easing, StyleProperty, Transition};
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
