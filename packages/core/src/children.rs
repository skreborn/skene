use std::{marker::PhantomData, rc::Rc};

use crate::{
  NodeId,
  tree::TreeOps,
  view::{AnyView, IntoView, View},
};

/// The absence of a child, in a view that requires one.
///
/// A wrapper or theme starts out holding this and cannot build; `set_child` exchanges
/// it for the child, and only then does the view exist as far as the type system is
/// concerned—so forgetting the child is a compile error, not an empty render.
pub struct NoChild;

/// The child kind a plain container accepts: any view.
pub struct PlainChild;

/// The child kind a flex container accepts: any view, or a `Flexible` wrapper.
pub struct FlexChild;

/// The child kind a stack accepts: any view, or a `Pinned` wrapper.
pub struct StackChild;

/// The child kind a grid accepts: any view, or a `GridCell` wrapper.
pub struct GridChild;

/// A value accepted as one child of a container of kind `Kind`.
///
/// Every view is a child of every kind; a facade's wrapper views (`Flexible`,
/// `Pinned`, `GridCell`) implement only the kind of the container that understands
/// their properties, so wrapping a child for the wrong container is a compile error.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a compatible and complete view",
  label = "not a compatible and complete view",
  note = "check that it is indeed a compatible view type",
  note = "see its documentation to determine what completes it"
)]
pub trait IntoChildOf<Kind> {
  /// Converts this value into the child's view.
  fn into_child(self) -> AnyView;
}

// Blaming this blanket buries the useful message: a value fails to be a child because
// it is not a view, and the reason it is not a view is what the author needs told.
#[diagnostic::do_not_recommend]
impl<Kind, V: IntoView> IntoChildOf<Kind> for V {
  #[inline(always)]
  fn into_child(self) -> AnyView {
    self.into_view()
  }
}

/// A value that can be converted into children of kind `Kind`.
///
/// Platform-agnostic: a facade builds its own children type from the resulting views.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a set of children",
  label = "not a set of children",
  note = "pass a tuple, `Vec`, or an array of children"
)]
pub trait IntoChildren<Kind = PlainChild> {
  /// Converts this value into children.
  fn into_children(self) -> Vec<AnyView>;
}

impl<Kind, V: IntoChildOf<Kind>> IntoChildren<Kind> for Vec<V> {
  #[inline(always)]
  fn into_children(self) -> Vec<AnyView> {
    self.into_iter().map(IntoChildOf::into_child).collect()
  }
}

impl<Kind, V: IntoChildOf<Kind>, const N: usize> IntoChildren<Kind> for [V; N] {
  #[inline(always)]
  fn into_children(self) -> Vec<AnyView> {
    self.into_iter().map(IntoChildOf::into_child).collect()
  }
}

macro_rules! impl_tuple {
  ($($T:ident),+) => {
    impl<Kind, $($T: IntoChildOf<Kind>),+> IntoChildren<Kind> for ($($T,)+) {
      #[inline(always)]
      fn into_children(self) -> Vec<AnyView> {
        #[expect(non_snake_case, clippy::min_ident_chars)]
        let ($($T,)+) = self;

        vec![$($T.into_child()),+]
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
  /// Creates a static set of children of kind `Kind`.
  #[inline(always)]
  pub fn new<Kind>(children: impl IntoChildren<Kind>) -> Self {
    Children(Inner::Static(children.into_children()), PhantomData)
  }
}

impl<Ops: TreeOps> Children<Ops> {
  /// Creates a derived set of children.
  ///
  /// `items` yields the items, `key` gives each a stable identity, `view` renders one.
  #[inline(always)]
  pub fn derive<Kind, T, I, K, V>(
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + std::hash::Hash + 'static,
    V: IntoChildOf<Kind>,
    Kind: 'static,
  {
    Children(
      Inner::Derived(Box::new(move |parent, prepare| {
        crate::reconcile::<Ops, _, _, _>(parent, items, key, move |item| {
          prepare(view(item).into_child().build())
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
  /// The child kind this container accepts; wrapper views carry per-child properties
  /// only the matching container understands.
  type Kind: 'static;

  /// Sets the children.
  ///
  /// This is the only method a trait implementor needs to provide.
  #[must_use]
  fn set_children_to(self, children: Children<Ops>) -> Self;

  /// Sets a single static child.
  #[inline(always)]
  #[must_use]
  fn set_child(self, child: impl IntoChildOf<Self::Kind>) -> Self {
    self.set_children_to(Children::new::<Self::Kind>(vec![child.into_child()]))
  }

  /// Sets a static set of children.
  #[inline(always)]
  #[must_use]
  fn set_children(self, children: impl IntoChildren<Self::Kind>) -> Self {
    self.set_children_to(Children::new(children))
  }

  /// Sets a single derived child.
  #[inline(always)]
  #[must_use]
  fn derive_child<V: IntoChildOf<Self::Kind>>(self, child: impl Fn() -> V + 'static) -> Self
  where
    Ops: TreeOps,
  {
    self.set_children_to(Children::derive::<Self::Kind, _, _, _, _>(
      || vec![()],
      |()| (),
      move |()| child(),
    ))
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
    V: IntoChildOf<Self::Kind>,
    Ops: TreeOps,
  {
    self.set_children_to(Children::derive::<Self::Kind, _, _, _, _>(items, key, view))
  }
}
