//! An optional setting applied in a builder chain.

use crate::Wraps;

/// Applies an optional setting: `.apply(value, Type::set_<prop>)` calls the setter where
/// there is a value and leaves the view as it is where there is none.
///
/// Every facade's view takes it, so a chain carries a setting it may lack without
/// breaking into an `if`.
///
/// ```
/// use skene_foundation::{Apply, Wraps};
///
/// #[derive(Debug, Default, PartialEq)]
/// struct Inner {
///   width: Option<f32>,
/// }
///
/// #[derive(Debug, Default, PartialEq, Wraps)]
/// struct Bar(Inner);
///
/// impl Bar {
///   fn set_width(mut self, width: f32) -> Self {
///     self.0.width = Some(width);
///     self
///   }
/// }
///
/// assert_eq!(
///   Bar::default().apply(Some(4.), Bar::set_width).0.width,
///   Some(4.)
/// );
/// assert_eq!(Bar::default().apply(None, Bar::set_width), Bar::default());
/// ```
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a facade's view",
  label = "`apply` sets a property of a facade's view",
  note = "a view takes `apply` by deriving `Wraps` over the foundation's view it holds"
)]
pub trait Apply: Sized {
  /// `set(self, value)` where `value` is `Some`, `self` otherwise.
  #[must_use]
  fn apply<T>(self, value: Option<T>, set: impl FnOnce(Self, T) -> Self) -> Self {
    match value {
      Some(value) => set(self, value),
      None => self,
    }
  }
}

#[diagnostic::do_not_recommend]
impl<T: Wraps> Apply for T {}
