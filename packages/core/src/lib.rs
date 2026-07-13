//! The shared foundation.

mod children;
mod node;
mod reorder;
mod runtime;
mod source;
mod values;
mod view;

pub use children::IntoChildren;
pub use node::NodeId;
pub use reorder::Reorder;
pub use runtime::{reset_root_scope, root_scope};
pub use source::Source;
pub use values::{
  Axis,
  Color,
  CrossAxisAlignment,
  Inset,
  MainAxisAlignment,
  MouseCursor,
  Point,
  Rect,
  Size,
};
pub use view::{AnyView, IntoView, View};
