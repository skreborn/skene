//! Common imports for a GUI application: the view vocabulary, value types, reactive
//! primitives, logging macros, and the entry point.

pub use skene_core::{
  Animation,
  Axis,
  BoxStyle,
  Color,
  Condition,
  CrossAxisAlignment,
  Easing,
  IntoChildren,
  IntoView,
  KeyEvent,
  MainAxisAlignment,
  MouseCursor,
  RunStyle,
  StyleProperty,
  Transition,
  View,
};
pub use skene_reactive::{
  Effect,
  IndexedField,
  KeyedField,
  Memo,
  ReadSignal,
  RwSignal,
  Scope,
  SignalGet,
  SignalUpdate,
  SignalWith,
  Store,
  StoreField,
  WriteSignal,
};
pub use tracing::{debug, error, info, trace, warn};

/// The entry point exists only on a target with a configured platform; on one without,
/// naming it is a compile error rather than a runtime panic. Each web leaf exposes its own—
/// the browser `launch`es, a server `render_to_string`s.
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub use crate::render_to_string;
pub use crate::{
  Anchor,
  Border,
  Button,
  Checkbox,
  Container,
  ErrorBoundary,
  Errors,
  FocusHandle,
  HasChildren,
  Input,
  Inset,
  Layout,
  Overlay,
  Point,
  Radio,
  Rect,
  RichText,
  Role,
  Select,
  Size,
  Slider,
  Span,
  Suspense,
  Text,
  TextStyle,
  TextTheme,
  Textarea,
  VirtualList,
  main,
};
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use crate::{hydrate_page, launch};
