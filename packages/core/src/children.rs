use crate::view::{AnyView, IntoView};

/// A value that can be converted into children.
///
/// Platform-agnostic: a facade builds its own children type from the resulting views.
pub trait IntoChildren {
  /// Converts this value into children.
  fn into_children(self) -> Vec<AnyView>;
}

impl<V: IntoView> IntoChildren for Vec<V> {
  #[inline(always)]
  fn into_children(self) -> Vec<AnyView> {
    self.into_iter().map(IntoView::into_view).collect()
  }
}

impl<V: IntoView, const N: usize> IntoChildren for [V; N] {
  #[inline(always)]
  fn into_children(self) -> Vec<AnyView> {
    self.into_iter().map(IntoView::into_view).collect()
  }
}

macro_rules! impl_tuple {
  ($($T:ident),+) => {
    impl<$($T: IntoView),+> IntoChildren for ($($T,)+) {
      #[inline(always)]
      fn into_children(self) -> Vec<AnyView> {
        #[expect(non_snake_case, clippy::min_ident_chars)]
        let ($($T,)+) = self;

        vec![$($T.into_view()),+]
      }
    }
  };
}

impl_tuple!(A);
impl_tuple!(A, B);
impl_tuple!(A, B, C);
impl_tuple!(A, B, C, D);
impl_tuple!(A, B, C, D, E);
impl_tuple!(A, B, C, D, E, F);
impl_tuple!(A, B, C, D, E, F, G);
impl_tuple!(A, B, C, D, E, F, G, H);
impl_tuple!(A, B, C, D, E, F, G, H, I);
impl_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);
