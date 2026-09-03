//! The one theme view: a scope defining a token set beneath it.

use std::marker::PhantomData;

use skene_core::next::{
  AnyKind,
  AnyView,
  Definitions,
  Driver,
  IntoView,
  NoChild,
  NodeId,
  NodeKind,
  Scene,
  TokenSet,
  View,
};
use skene_reactive::{Effect, Memo, Owner, With, provide_context};

/// A palette: fixed, or recomputed from the signals it reads.
enum Palette<P> {
  Fixed(P),
  Derived(Box<dyn Fn() -> P>),
}

/// The closure building a theme's child under the theme's owner.
#[must_use]
pub struct ChildBuilder<D: Driver>(Box<dyn FnOnce() -> AnyView<D>>);

/// Defines the token set `P` for everything beneath it.
///
/// Realizes one layout-invisible scope node, provides `Memo<P>` through context so
/// `P::current()` and `P::watch()` read it, and rewrites the scope's definitions when
/// the palette changes: no consumer is touched and nothing rebuilds. Nesting a theme
/// redefines beneath it. Holds [`NoChild`] until
/// [`set_child_builder`](Self::set_child_builder) supplies the child; the child is a
/// builder closure because it is built under the owner the context is provided on.
#[must_use]
pub struct Theme<D: Driver, P: TokenSet<D::TokenValue>, C = NoChild> {
  palette: Palette<P>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver, P: TokenSet<D::TokenValue>> Theme<D, P> {
  /// A theme defining `palette`.
  #[inline(always)]
  pub fn new(palette: P) -> Self {
    Self {
      palette: Palette::Fixed(palette),
      child: NoChild,
      driver: PhantomData,
    }
  }

  /// A theme defining the palette `palette` returns, recomputed whenever a signal it
  /// reads changes.
  #[inline(always)]
  pub fn derive(palette: impl Fn() -> P + 'static) -> Self {
    Self {
      palette: Palette::Derived(Box::new(palette)),
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver, P: TokenSet<D::TokenValue>, C> Theme<D, P, C> {
  /// Sets the child, built by `child` under the theme.
  #[inline(always)]
  pub fn set_child_builder<V: IntoView<D>>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> Theme<D, P, ChildBuilder<D>> {
    Theme {
      palette: self.palette,
      child: ChildBuilder(Box::new(move || child().into_view())),
      driver: PhantomData,
    }
  }
}

impl<D: Driver, P: TokenSet<D::TokenValue>, C> AnyKind for Theme<D, P, C> {}

impl<D: Driver, P: TokenSet<D::TokenValue>> View<D> for Theme<D, P, ChildBuilder<D>> {
  fn build(self) -> NodeId {
    let driver = D::default();
    let scope = Scene::<D>::with(|scene| scene.create(NodeKind::Scope, driver.create_scope()));
    let owner = Owner::current().child();
    let ChildBuilder(child) = self.child;
    let palette = self.palette;

    owner.enter(|| {
      let memo = match palette {
        Palette::Fixed(palette) => Memo::new(move |_| palette.clone()),
        Palette::Derived(palette) => Memo::new(move |_| palette()),
      };

      provide_context(memo);

      Effect::render(move |_: Option<()>| {
        let definitions = memo.with(Definitions::of);

        Scene::<D>::with(|scene| scene.define_tokens(scope, definitions));
      });

      let child = child().build();

      Scene::<D>::with(|scene| scene.attach(scope, child, None));
    });

    scope
  }
}
