//! What a view is, and what composing views takes.

use std::{
  cell::{Cell, RefCell},
  hash::Hash,
};

use skene_foundation::{
  Effect, Keyed, NodeId, Signal,
  seam::{
    node::{NodeKind, NodeSpec},
    record::Record,
    style::ScopeChain,
    tree,
  },
  untrack,
};

use crate::Active;

/// A piece of UI, realized once into one node of the scene.
///
/// Most code never implements it: `fn card() -> impl View` returns a composition of the
/// built-in views. A library's own view implements it by building what it is made of:
///
/// ```
/// use skene_gui::prelude::*;
///
/// struct Badge {
///   label: String,
/// }
///
/// impl View for Badge {
///   fn build(self, cx: &mut Cx) -> Node {
///     Container::new()
///       .set_padding(4)
///       .set_child(Text::new(self.label))
///       .build(cx)
///   }
/// }
/// ```
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a skene-gui view",
  label = "expected a skene-gui view here",
  note = "a skene-gui view is one of its views (`Container`, `Text`, `Toggle`, ...) or a type implementing `skene_gui::View`"
)]
pub trait View: 'static {
  /// Realizes this view in `cx`, returning the node it realized.
  fn build(self, cx: &mut Cx) -> Node;

  /// This view with its type erased, so views of different types can stand in one place.
  fn into_any(self) -> AnyView
  where
    Self: Sized,
  {
    AnyView::new(self)
  }
}

/// A child a layout of type `L` takes: any view, and the wrapper that places a child in that
/// layout (`Flexible` in a `Row` or a `Column`, `GridCell` in a `Grid`, `Pinned` in a
/// `Stack`).
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a child a `{L}` takes",
  label = "not a child of a `{L}`",
  note = "any view goes in any layout; `Flexible` goes in a `Row` or a `Column`, `GridCell` in a `Grid`, `Pinned` in a `Stack`"
)]
pub trait ChildOf<L>: 'static {
  /// Realizes this child in `cx`, its placement folded into its own properties.
  fn build_child(self, cx: &mut Cx) -> Node;
}

#[diagnostic::do_not_recommend]
impl<V: View, L> ChildOf<L> for V {
  #[inline(always)]
  fn build_child(self, cx: &mut Cx) -> Node {
    self.build(cx)
  }
}

/// A realized node.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Node(pub(crate) NodeId);

impl Node {
  /// The scene's id of the node.
  #[inline(always)]
  #[must_use]
  pub const fn id(self) -> NodeId {
    self.0
  }
}

/// Where a view is built: the theme scopes it sits in, and the placement the wrappers above it
/// fold into its properties. Passed along, never made by a view.
pub struct Cx {
  /// The theme scopes the view sits in, innermost first.
  pub(crate) scope: ScopeChain,
  /// What the wrappers above the view fold into its own properties.
  pub(crate) placement: Record,
}

impl Cx {
  /// The context an app's root is built in: no theme, no placement.
  pub(crate) const fn root() -> Self {
    Self {
      scope: ScopeChain::root(),
      placement: Record::new(),
    }
  }

  /// The context a view's children are built in: the same scopes, no placement.
  pub(crate) fn child(&self) -> Self {
    Self {
      scope: self.scope.clone(),
      placement: Record::new(),
    }
  }
}

/// A slot a built node's id is written into, so a relation can name it: a control named by a
/// caption placed anywhere (`set_labelled_by`), before or after it.
///
/// It is a signal: what reads it ([`NodeHandle::node`]) follows it, so a relation to a node
/// built later is made once that node is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct NodeHandle(Signal<Option<NodeId>>);

impl Default for NodeHandle {
  fn default() -> Self {
    Self(Signal::new(None))
  }
}

impl NodeHandle {
  /// An empty handle.
  pub fn new() -> Self {
    Self::default()
  }

  /// The node, once built, subscribing the running effect.
  #[must_use]
  pub fn node(&self) -> Option<NodeId> {
    self.0.get()
  }

  /// Writes `id` into the handle: what a view does once it built its node.
  pub(crate) fn set(&self, id: NodeId) {
    self.0.set(Some(id));
  }
}

/// A view whose type is erased.
#[must_use]
pub struct AnyView(Box<dyn FnOnce(&mut Cx) -> Node>);

impl AnyView {
  /// `view`, its type erased.
  pub fn new(view: impl View) -> Self {
    Self(Box::new(move |cx| view.build(cx)))
  }
}

impl View for AnyView {
  fn build(self, cx: &mut Cx) -> Node {
    (self.0)(cx)
  }

  fn into_any(self) -> AnyView {
    self
  }
}

/// Its view where there is one, and nothing where there is none: a node that takes no room
/// and no spacing in its parent's layout.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let wide = media::watch(Media::MinWidth(720));
/// let details = Derived::new(move || wide.get().then(|| Text::new("Details")));
/// ```
impl<V: View> View for Option<V> {
  fn build(self, cx: &mut Cx) -> Node {
    match self {
      Some(view) => view.build(cx),
      None => Node(tree::create::<Active>(
        NodeSpec::new(NodeKind::Segment),
        &cx.scope,
      )),
    }
  }
}

/// A child not built yet: what a view holds until it is built.
pub(crate) type Child = Box<dyn FnOnce(&mut Cx) -> Node>;

/// `child` held to be built later, as a child of a `L`.
pub(crate) fn child_of<L>(child: impl ChildOf<L>) -> Child {
  Box::new(move |cx| child.build_child(cx))
}

/// Gives a created node its record, the placement `cx` carries folded in, and writes it into
/// `handle`.
pub(crate) fn finish(id: NodeId, mut record: Record, cx: &Cx, handle: Option<&NodeHandle>) {
  record.fold(&cx.placement);
  tree::set_record::<Active>(id, record);

  if let Some(handle) = handle {
    handle.set(id);
  }
}

/// Builds `children` in order under `parent` and attaches each.
pub(crate) fn attach_children(parent: NodeId, children: Vec<Child>, cx: &Cx) {
  for (index, child) in children.into_iter().enumerate() {
    let node = child(&mut cx.child());

    tree::attach::<Active>(parent, node.0, index);
  }
}

/// A keyed list in a layout: a segment whose children are one view per item, each built once
/// per key under an owner of its own and kept, wherever it moves, while its key stays.
pub(crate) fn keyed<T: 'static, K: Eq + Hash + Clone + 'static, C: ChildOf<L>, L>(
  items: impl Fn() -> Vec<T> + 'static,
  key: impl Fn(&T) -> K + 'static,
  view: impl Fn(T) -> C + 'static,
) -> Child {
  Box::new(move |cx| {
    let segment = tree::create::<Active>(NodeSpec::new(NodeKind::Segment), &cx.scope);
    let scope = cx.scope.clone();
    let list = RefCell::new(Keyed::<K, NodeId>::new());

    Effect::new(move || {
      let items = items();
      let ids = untrack(|| {
        list.borrow_mut().update(
          items,
          &key,
          |item| {
            view(item)
              .build_child(&mut Cx {
                scope: scope.clone(),
                placement: Record::new(),
              })
              .0
          },
          tree::remove::<Active>,
        )
      });

      untrack(|| {
        for (index, id) in ids.iter().enumerate() {
          tree::attach::<Active>(segment, *id, index);
        }
      });
    });

    Node(segment)
  })
}

/// The view a closure returns, rebuilt in place whenever a signal the closure read changes;
/// what the previous one created is disposed with it.
///
/// It mounts under an anchor of its own, so its view joins its parent's layout where the
/// derived view stands.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let signed_in = Signal::new(false);
/// let greeting = Derived::new(move || match signed_in.get() {
///   true => Text::new("Welcome back").into_any(),
///   false => Button::new("Sign in").into_any(),
/// });
/// ```
#[must_use]
pub struct Derived {
  build: Box<dyn Fn() -> AnyView>,
}

impl Derived {
  /// The view `build` returns, kept current.
  pub fn new<V: View>(build: impl Fn() -> V + 'static) -> Self {
    Self {
      build: Box::new(move || build().into_any()),
    }
  }
}

impl View for Derived {
  fn build(self, cx: &mut Cx) -> Node {
    let segment = tree::create::<Active>(NodeSpec::new(NodeKind::Segment), &cx.scope);
    let (scope, placement) = (cx.scope.clone(), cx.placement.clone());
    let shown = Cell::new(None::<NodeId>);
    let build = self.build;

    Effect::new(move || {
      let view = build();

      untrack(|| {
        let node = view.build(&mut Cx {
          scope: scope.clone(),
          placement: placement.clone(),
        });

        tree::attach::<Active>(segment, node.0, 0);

        if let Some(old) = shown.replace(Some(node.0)) {
          tree::remove::<Active>(old);
        }
      });
    });

    Node(segment)
  }
}

/// A property's `set_` and `derive_` setters, writing the record under its uniform key.
///
/// A platform's own property is an entry of a `linux { .. }` block (`set_linux_*`): its setters
/// and the view's `linux` field holding them are compiled in that platform's build alone, and
/// the platform's realization applies it.
macro_rules! properties {
  (linux { $($(#[doc = $doc:literal])* $set:ident, $derive:ident: $field:ident, $ty:ty;)* }) => {$(
    $(#[doc = $doc])*
    #[cfg(skene_platform = "linux")]
    pub fn $set(self, value: impl Into<$ty>) -> Self {
      let value: $ty = value.into();

      self.$derive(move || value.clone())
    }

    $(#[doc = $doc])*
    ///
    /// Derived: recomputed whenever a signal the closure reads changes.
    #[cfg(skene_platform = "linux")]
    pub fn $derive<V: Into<$ty>>(mut self, value: impl Fn() -> V + 'static) -> Self {
      self.linux.$field = Some(::std::rc::Rc::new(move || value().into()));

      self
    }
  )*};
  ($($(#[doc = $doc:literal])* $set:ident, $derive:ident: $key:ident, $ty:ty;)*) => {$(
    $(#[doc = $doc])*
    pub fn $set(mut self, value: impl Into<::skene_foundation::Value<$ty>>) -> Self {
      self
        .record
        .set(::skene_foundation::seam::record::keys::$key, value);

      self
    }

    $(#[doc = $doc])*
    ///
    /// Derived: recomputed whenever a signal the closure reads changes.
    pub fn $derive<V: Into<::skene_foundation::Value<$ty>>>(
      mut self,
      value: impl Fn() -> V + 'static,
    ) -> Self {
      self.record.set(
        ::skene_foundation::seam::record::keys::$key,
        ::skene_foundation::Value::derived(value),
      );

      self
    }
  )*};
}

pub(crate) use properties;

/// Applies an optional setting: `.apply(self.change, Switch::handle_change)` calls the setter
/// where there is a value and leaves the view as it is where there is none.
pub trait Apply: Sized {
  /// `set(self, value)` where `value` is `Some`, `self` otherwise.
  fn apply<T>(self, value: Option<T>, set: impl FnOnce(Self, T) -> Self) -> Self {
    match value {
      Some(value) => set(self, value),
      None => self,
    }
  }
}

impl<V: View> Apply for V {}

/// A view's `set_handle`, writing its `handle` field.
macro_rules! handle {
  () => {
    /// Writes the view's node into `handle` once built.
    pub const fn set_handle(mut self, handle: &NodeHandle) -> Self {
      self.handle = Some(*handle);

      self
    }
  };
}

pub(crate) use handle;
