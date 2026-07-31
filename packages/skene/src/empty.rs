/// A view that renders nothing.
///
/// A no-op placeholder—for example, the branch a view returns for a platform it has no
/// implementation for.
#[cfg(any(target_arch = "wasm32", feature = "tui"))]
#[inline(always)]
#[must_use]
pub fn empty() -> impl skene_core::View {
  skene_core::AnyView::new(crate::driver::create_group)
}
