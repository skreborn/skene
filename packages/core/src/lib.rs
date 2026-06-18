//! The shared foundation.

mod backend;
mod children;
#[cfg(any(test, feature = "test"))]
mod mock;
mod node;
mod reconcile;
mod runtime;
mod source;
mod values;
mod view;

pub use backend::Backend;
pub use children::{Children, HasChildren, IntoChildren};
#[cfg(any(test, feature = "test"))]
pub use mock::{MockBackend, MockOp};
pub use node::NodeId;
pub use reconcile::{dynamic, reconcile};
#[cfg(any(test, feature = "test"))]
pub use runtime::{BackendGuard, install_test_backend};
pub use runtime::{install_backend, mount, root_scope, with_backend, with_backend_as};
pub use source::Source;
pub use values::{Axis, Color, CrossAxisAlignment, Inset, MainAxisAlignment, MouseCursor, Number};
pub use view::{AnyView, DynamicView, IntoView, View};
