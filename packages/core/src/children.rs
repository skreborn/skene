use std::{marker::PhantomData, rc::Rc};

use crate::{
  NodeId,
  tree::TreeOps,
  view::{AnyView, IntoView, View},
};

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

/// A container's children, generic over the facade's [`TreeOps`].
///
/// A facade aliases this with its own ops type, so app code sees a plain `Children`.
#[must_use]
pub struct Children<Ops>(Inner, PhantomData<Ops>);

enum Inner {
  Static(Vec<AnyView>),
  Derived(Box<dyn FnOnce(NodeId, Prepare)>),
}

type Prepare = Rc<dyn Fn(NodeId) -> NodeId>;

impl<Ops> Default for Children<Ops> {
  #[inline(always)]
  fn default() -> Self {
    Children(Inner::Static(Vec::new()), PhantomData)
  }
}

impl<Ops> Children<Ops> {
  /// Creates a static set of children.
  #[inline(always)]
  pub fn new(children: impl IntoChildren) -> Self {
    Children(Inner::Static(children.into_children()), PhantomData)
  }
}

impl<Ops: TreeOps> Children<Ops> {
  /// Creates a derived set of children.
  ///
  /// `items` yields the items, `key` gives each a stable identity, `view` renders one.
  #[inline(always)]
  pub fn derive<T, I, K, V>(
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + std::hash::Hash + 'static,
    V: View,
  {
    Children(
      Inner::Derived(Box::new(move |parent, prepare| {
        crate::reconcile::<Ops, _, _, _>(parent, items, key, move |item| {
          prepare(view(item).build())
        });
      })),
      PhantomData,
    )
  }

  /// Mounts these children under `parent`.
  pub fn mount(self, parent: NodeId) {
    self.mount_with(parent, std::convert::identity);
  }

  /// Mounts these children under `parent`.
  ///
  /// Each built child node is passed through `prepare` before insertion.
  ///
  /// Reconciliation tracks the node `prepare` returns.
  pub fn mount_with(self, parent: NodeId, prepare: impl Fn(NodeId) -> NodeId + 'static) {
    let prepare: Prepare = Rc::new(prepare);

    match self.0 {
      Inner::Static(views) => {
        for view in views {
          let node = prepare(view.build());

          Ops::default().attach(parent, node, None);
        }
      }
      Inner::Derived(install) => install(parent, prepare),
    }
  }
}

/// A container that has children.
pub trait HasChildren<Ops>: Sized {
  /// Sets the children.
  ///
  /// This is the only method a trait implementor needs to provide.
  #[must_use]
  fn set_children_to(self, children: Children<Ops>) -> Self;

  /// Sets a single static child.
  #[inline(always)]
  #[must_use]
  fn set_child(self, child: impl IntoView) -> Self {
    self.set_children_to(Children::new(vec![child.into_view()]))
  }

  /// Sets a static set of children.
  #[inline(always)]
  #[must_use]
  fn set_children(self, children: impl IntoChildren) -> Self {
    self.set_children_to(Children::new(children))
  }

  /// Sets a single derived child.
  #[inline(always)]
  #[must_use]
  fn derive_child<V: View>(self, child: impl Fn() -> V + 'static) -> Self
  where
    Ops: TreeOps,
  {
    self.set_children_to(Children::derive(|| vec![()], |()| (), move |()| child()))
  }

  /// Sets a derived set of children.
  ///
  /// `items` yields the items, `key` gives each a stable identity, `view` renders one.
  #[inline(always)]
  #[must_use]
  fn derive_children<T, I, K, V>(
    self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + std::hash::Hash + 'static,
    V: View,
    Ops: TreeOps,
  {
    self.set_children_to(Children::derive(items, key, view))
  }
}
