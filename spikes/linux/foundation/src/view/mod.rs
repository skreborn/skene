//! The view layer every facade shares, written once over the seam: what a view is ([`View`]),
//! where it is built ([`Cx`]), and every built-in view, each generic over the realization `R`
//! it is built for and each member written once, in a trait a facade's own type takes it
//! from.
//!
//! **A facade declares a type of its own per view**: its documentation, its fields one
//! foundation view (`pub struct Container(view::Container<Active>)`), [`Wraps`] and [`View`]
//! implemented by [`wraps!`](crate::wraps!), and a line per trait it takes its members from
//! (`impl ContainerBase<Active> for Container {}`, `impl DecorationBase for Container {}`).
//! Its own members are inherent beside them (a part's default, which returns the facade's own
//! type), and the rest of the surface both facades share is declared by one
//! [`facade!`](crate::facade!), so app code writes `impl View` with no type parameter.
//!
//! **A view's own members** live in its `<View>Base` trait ([`ContainerBase`],
//! [`ControlBase`]); a surface several views share in a trait of its own ([`DecorationBase`],
//! [`RunBase`], [`MotionBase`], [`ControlMembersBase`]), and what a capability adds in a
//! `<Trait>With<Capability>` one ([`RunWithEntries`]). Each is implemented by the foundation's
//! view too, which wraps itself, so the built-ins compose one another through the same
//! members an app calls.
//!
//! **Nothing on the path a built-in takes is private**: a library's view creates its nodes in
//! a [`Cx`] ([`Cx::create`], [`Cx::finish`], [`Cx::attach_children`]) exactly as the
//! built-ins do, and its drawn control is a [`Control`] over a behavior of its own.

mod access;
mod control;
pub mod controls;
mod overlay;
pub mod painted;
mod surface;
mod views;
mod wrappers;

use std::{
  cell::{Cell, RefCell},
  hash::Hash,
  marker::PhantomData,
};

pub use self::{
  access::Access,
  control::{
    AsPart, AsPartBase, BasicButton, BasicCheckbox, BasicListbox, BasicListboxOption, BasicPicker,
    BasicRadio, BasicRadioGroup, BasicSlider, BasicTextEditor, BasicTextField, Control,
    ControlBase, ControlMembersBase, HasMembers, Label, Members,
  },
  controls::{
    Button, ButtonBase, Checkbox, CheckboxBase, ChoiceBase, ContextMenu, ContextMenuBase,
    EditingBase, GroupOptions, HasChoice, HasEntry, HasOptions, Keyboard, Listbox, ListboxBase,
    ListboxItem, ListboxItemBase, MenuEntry, MenuItem, MenuItemBase, OptionSegment, OptionsBase,
    Picker, PickerBase, PickerItem, PickerItemBase, Radio, RadioBase, RadioGroup, RadioGroupBase,
    Slider, SliderBase, TextEditor, TextEditorBase, TextField, TextFieldBase, Toggle, ToggleBase,
  },
  overlay::{Overlay, OverlayBase},
  painted::{Canvas, Paint, Painted, PaintedBase, Painting},
  surface::{DecorationBase, MotionBase, RunBase, RunWithEntries},
  views::{
    Column, ColumnBase, Container, ContainerBase, Grid, GridBase, Image, ImageBase, Row, RowBase,
    Scroll, ScrollAxes, ScrollBase, Stack, StackBase, Text, TextBase, Theme, ThemeBase,
  },
  wrappers::{Flexible, FlexibleBase, GridCell, GridCellBase, Pinned, PinnedBase},
};
use crate::{
  Effect, Keyed, NodeId, Signal,
  seam::{
    Realization,
    access::Role,
    control::Part,
    node::{Interaction, NodeKind, NodeSpec},
    record::Record,
    style::ScopeChain,
    tree,
  },
  untrack,
};

/// A piece of UI, realized once into one node of the scene, for the realization `R`.
///
/// Most code never implements it: a function returns a composition of the built-in views. A
/// library's own view implements it for its facade's realization by building what it is made
/// of, as a built-in does.
///
/// A view also implements [`AnyKind`], which a wrapper does not.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a view of this build's platform",
  label = "expected a view here",
  note = "a view is one of the facade's views (`Container`, `Text`, `Toggle`, ...) or a type implementing `skene_foundation::view::View` for the facade's realization"
)]
pub trait View<R: Realization>: AnyKind + 'static {
  /// Realizes this view in `cx`, returning the node it realized.
  fn build(self, cx: &mut Cx<R>) -> Node;

  /// This view with its type erased, so views of different types can stand in one place.
  fn into_any(self) -> AnyView<R>
  where
    Self: Sized,
  {
    AnyView::new(self)
  }
}

/// A view, accepted as a child of every layout: every view implements it, and a wrapper
/// carrying per-child properties (`Flexible`, `Pinned`, `GridCell`) does not, implementing
/// [`ChildOf`] for the layouts that understand them instead.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a view of every kind",
  label = "a view implements `AnyKind`; a wrapper implements `ChildOf` for its layouts instead"
)]
pub trait AnyKind {}

/// A child a layout of type `L` takes: any view, and the wrapper that places a child in that
/// layout (`Flexible` in a `Row` or a `Column`, `GridCell` in a `Grid`, `Pinned` in a
/// `Stack`).
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a child a `{L}` takes",
  label = "not a child of a `{L}`",
  note = "any view goes in any layout; `Flexible` goes in a `Row` or a `Column`, `GridCell` in a `Grid`, `Pinned` in a `Stack`"
)]
pub trait ChildOf<L, R: Realization>: 'static {
  /// Realizes this child in `cx`, its placement folded into its own properties.
  fn build_child(self, cx: &mut Cx<R>) -> Node;
}

#[diagnostic::do_not_recommend]
impl<V: View<R>, L, R: Realization> ChildOf<L, R> for V {
  #[inline(always)]
  fn build_child(self, cx: &mut Cx<R>) -> Node {
    self.build(cx)
  }
}

/// A view type over a foundation view: a facade's own type over the foundation's (its one
/// field), or the foundation's view itself, which wraps itself. What the members of the
/// `*Base` traits reach the view through.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is no view type over a foundation view",
  note = "a facade's own view holds one foundation view and implements `Wraps` through `skene_foundation::wraps!`"
)]
pub trait Wraps: Sized {
  /// The foundation's view.
  type Inner;

  /// `inner`, as this type.
  fn wrap(inner: Self::Inner) -> Self;

  /// The foundation's view.
  fn inner(&self) -> &Self::Inner;

  /// The foundation's view, to change in place.
  fn inner_mut(&mut self) -> &mut Self::Inner;

  /// The foundation's view, given up.
  fn into_inner(self) -> Self::Inner;

  /// This view with its foundation view changed by `map`.
  #[must_use]
  fn map_inner(self, map: impl FnOnce(Self::Inner) -> Self::Inner) -> Self {
    Self::wrap(map(self.into_inner()))
  }
}

/// Implements [`Wraps`] (and [`View`] for the realization named) for a facade's own type
/// whose one field is a foundation view: `wraps!([] Container => view::Container<Active>,
/// Active)`, its generic parameters in the brackets (`[T: Choice] Picker<T> => ..`). A wrapper,
/// which is no view, takes `wrapper` first and implements [`ChildOf`] for its layouts itself.
#[macro_export]
macro_rules! wraps {
  (wrapper [$($generics:tt)*] $name:ty => $inner:ty) => {
    impl<$($generics)*> $crate::view::Wraps for $name {
      type Inner = $inner;

      #[inline(always)]
      fn wrap(inner: $inner) -> Self {
        Self(inner)
      }

      #[inline(always)]
      fn inner(&self) -> &$inner {
        &self.0
      }

      #[inline(always)]
      fn inner_mut(&mut self) -> &mut $inner {
        &mut self.0
      }

      #[inline(always)]
      fn into_inner(self) -> $inner {
        self.0
      }
    }
  };
  ([$($generics:tt)*] $name:ty => $inner:ty, $realization:ty) => {
    $crate::wraps!(wrapper [$($generics)*] $name => $inner);

    impl<$($generics)*> $crate::view::AnyKind for $name {}

    impl<$($generics)*> $crate::view::View<$realization> for $name {
      #[inline(always)]
      fn build(self, cx: &mut $crate::view::Cx<$realization>) -> $crate::view::Node {
        $crate::view::View::build(self.0, cx)
      }
    }
  };
}

/// Implements [`Wraps`] for a foundation view over itself.
macro_rules! wraps_itself {
  ($name:ident $(<$($generics:ident),*>)? where $($bounds:tt)*) => {
    impl<$($($generics),*)?> $crate::view::Wraps for $name $(<$($generics),*>)?
    where
      $($bounds)*
    {
      type Inner = Self;

      #[inline(always)]
      fn wrap(inner: Self) -> Self {
        inner
      }

      #[inline(always)]
      fn inner(&self) -> &Self {
        self
      }

      #[inline(always)]
      fn inner_mut(&mut self) -> &mut Self {
        self
      }

      #[inline(always)]
      fn into_inner(self) -> Self {
        self
      }
    }
  };
}

use wraps_itself;

/// Declares the surface a facade shares with every other over its realization `$realization`:
/// its `View` (every view of the realization, its `build` and `into_any` the foundation's
/// [`View`]'s, which the facade's prelude brings into scope unnamed) and the names of the
/// foundation's types it takes as they are: `Cx`, `AnyView`, `Label` and `Painting`.
#[macro_export]
macro_rules! facade {
  ($realization:ty) => {
    /// A view on this build's platform: one of the facade's views, or a library's own type
    /// implementing `skene_foundation::view::View` for this build's realization. Its `build`
    /// and `into_any` are that trait's.
    pub trait View: $crate::view::View<$realization> {}

    impl<V: $crate::view::View<$realization>> View for V {}

    /// Where a view is built: the theme scopes it sits in, the placement the wrappers above
    /// it fold into its properties, and the control it is built inside.
    pub type Cx = $crate::view::Cx<$realization>;

    /// A view whose type is erased.
    pub type AnyView = $crate::view::AnyView<$realization>;

    /// A control's caption: a view built as its label part, with the name it supplies where
    /// known; a text is named by its text.
    pub type Label = $crate::view::Label<$realization>;

    /// Whether the platform paints a `Painted` box now.
    pub type Painting = $crate::view::Painting<$realization>;
  };
}

/// What a view writes its properties into: one record, its own or its control's root's.
pub trait HasRecord {
  /// The record.
  fn record_mut(&mut self) -> &mut Record;
}

/// A property's `set_` and `derive_` setters on a trait over [`Wraps`], writing the record its
/// view keeps ([`HasRecord`]) under the property's uniform key.
macro_rules! trait_properties {
  ($($(#[doc = $doc:literal])* $set:ident, $derive:ident: $key:ident, $ty:ty;)*) => {$(
    $(#[doc = $doc])*
    fn $set(mut self, value: impl Into<$crate::Value<$ty>>) -> Self {
      $crate::view::HasRecord::record_mut(self.inner_mut())
        .set($crate::seam::record::keys::$key, value);

      self
    }

    $(#[doc = $doc])*
    ///
    /// Derived: recomputed whenever a signal the closure reads changes.
    fn $derive<V: Into<$crate::Value<$ty>>>(mut self, value: impl Fn() -> V + 'static) -> Self {
      $crate::view::HasRecord::record_mut(self.inner_mut()).set(
        $crate::seam::record::keys::$key,
        $crate::Value::derived(value),
      );

      self
    }
  )*};
}

use trait_properties;

/// A realized node.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Node(NodeId);

impl Node {
  /// The node `id`, as the view that created it returns it.
  #[inline(always)]
  #[must_use]
  pub const fn new(id: NodeId) -> Self {
    Self(id)
  }

  /// The scene's id of the node.
  #[inline(always)]
  #[must_use]
  pub const fn id(self) -> NodeId {
    self.0
  }
}

/// Where a view is built: the theme scopes it sits in, the placement the wrappers above it
/// fold into its properties, the control it is built inside, what the part wrapping it makes
/// of it, and how its node takes events where the view building it says. Passed along; a view
/// makes one for its children ([`Cx::child`]).
pub struct Cx<R: Realization> {
  scope: ScopeChain,
  placement: Record,
  control: Option<NodeId>,
  part: Option<Part>,
  presentational: bool,
  interaction: Option<Interaction>,
  realization: PhantomData<fn() -> R>,
}

impl<R: Realization> Cx<R> {
  /// The context an app's root is built in: no theme, no placement, no control.
  #[must_use]
  pub const fn root() -> Self {
    Self {
      scope: ScopeChain::root(),
      placement: Record::new(),
      control: None,
      part: None,
      presentational: false,
      interaction: None,
      realization: PhantomData,
    }
  }

  /// The context a view's children are built in: the same scopes and control, no placement
  /// and no part.
  #[must_use]
  pub fn child(&self) -> Self {
    Self {
      scope: self.scope.clone(),
      placement: Record::new(),
      control: self.control,
      part: None,
      presentational: false,
      interaction: None,
      realization: PhantomData,
    }
  }

  /// A context like this one: what a view that builds another in its place (a derived view, a
  /// wrapper) hands it.
  #[must_use]
  pub fn inherit(&self) -> Self {
    Self {
      scope: self.scope.clone(),
      placement: self.placement.clone(),
      control: self.control,
      part: self.part,
      presentational: self.presentational,
      interaction: self.interaction,
      realization: PhantomData,
    }
  }

  /// The theme scopes the view sits in, innermost first.
  #[must_use]
  pub const fn scope(&self) -> &ScopeChain {
    &self.scope
  }

  /// Builds what follows in `scope`: a theme's child.
  pub fn set_scope(&mut self, scope: ScopeChain) {
    self.scope = scope;
  }

  /// The control the view is built inside, whose states its nodes resolve against.
  #[must_use]
  pub const fn control(&self) -> Option<NodeId> {
    self.control
  }

  /// Builds what follows inside the control `control`: a control's parts and body.
  pub const fn set_control(&mut self, control: NodeId) {
    self.control = Some(control);
  }

  /// Makes the next node created the control's `part`, a reader skipping it where the part is
  /// decoration ([`Part::is_presentational`]).
  pub const fn set_part(&mut self, part: Part) {
    self.part = Some(part);
    self.presentational = part.is_presentational();
  }

  /// Whether a reader skips the next node created and everything inside it.
  pub const fn set_presentational(&mut self, presentational: bool) {
    self.presentational = presentational;
  }

  /// Makes the next node created take events as `interaction` says, in place of its own: a
  /// caption beside its control ([`Interaction::Answering`]), a press on which focuses the
  /// control and whose hover and press the control takes, or a scroll bar's track
  /// ([`Interaction::ScrollBar`]).
  pub const fn set_interaction(&mut self, interaction: Interaction) {
    self.interaction = Some(interaction);
  }

  /// Folds `record` into the placement the next node created takes: a wrapper's properties.
  pub fn fold_placement(&mut self, record: &Record) {
    self.placement.fold(record);
  }

  /// Creates the view's node of `spec` in this context: in its scopes, inside its control,
  /// as its part, skipped by a reader or answering a press where the context says. A segment
  /// passes the part on to the node built beneath it.
  pub fn create(&mut self, mut spec: NodeSpec) -> NodeId {
    if spec.kind == NodeKind::Segment {
      return tree::create::<R>(spec, &self.scope, self.control);
    }

    if std::mem::take(&mut self.presentational) {
      spec = spec.with_role(Role::Presentation);
    }

    if let Some(interaction) = self.interaction.take() {
      spec = spec.with_interaction(interaction);
    }

    let id = tree::create::<R>(spec, &self.scope, self.control);

    if let Some(part) = self.part.take() {
      tree::set_part::<R>(id, part);
    }

    id
  }

  /// Gives the created node `id` its record, the placement this context carries folded in,
  /// and writes it into `handle`.
  pub fn finish(&self, id: NodeId, mut record: Record, handle: Option<&NodeHandle>) {
    record.fold(&self.placement);
    tree::set_record::<R>(id, record);

    if let Some(handle) = handle {
      handle.set(id);
    }
  }

  /// Builds `children` in order under `parent`, each in a child context, and attaches each.
  pub fn attach_children(&self, parent: NodeId, children: Vec<Child<R>>) {
    for (index, child) in children.into_iter().enumerate() {
      let node = child.build(&mut self.child());

      tree::attach::<R>(parent, node.id(), index);
    }
  }
}

/// What builds a child in the context it is built in.
type Build<R> = dyn FnOnce(&mut Cx<R>) -> Node;

/// A child not built yet: what a view holds until it is built.
#[must_use]
pub struct Child<R: Realization>(Box<Build<R>>);

impl<R: Realization> Child<R> {
  /// The child `build` builds.
  pub fn new(build: impl FnOnce(&mut Cx<R>) -> Node + 'static) -> Self {
    Self(Box::new(build))
  }

  /// `child`, held to be built later as a child of a `L`.
  pub fn of<L>(child: impl ChildOf<L, R>) -> Self {
    Self::new(move |cx| child.build_child(cx))
  }

  /// `view`, held to be built later.
  pub fn view(view: impl View<R>) -> Self {
    Self::new(move |cx| view.build(cx))
  }

  /// A keyed list in a layout of type `L`: a segment whose children are one view per item,
  /// each built once per key under an owner of its own and kept, wherever it moves, while its
  /// key stays.
  pub fn keyed<T: 'static, K: Eq + Hash + Clone + 'static, C: ChildOf<L, R>, L>(
    items: impl Fn() -> Vec<T> + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> C + 'static,
  ) -> Self {
    Self::new(move |cx| {
      let segment = cx.create(NodeSpec::new(NodeKind::Segment));
      let inner = cx.child();
      let list = RefCell::new(Keyed::<K, NodeId>::new());

      Effect::new(move || {
        let items = items();
        let ids = untrack(|| {
          list.borrow_mut().update(
            items,
            &key,
            |item| view(item).build_child(&mut inner.child()).id(),
            tree::remove::<R>,
          )
        });

        untrack(|| {
          for (index, id) in ids.iter().enumerate() {
            tree::attach::<R>(segment, *id, index);
          }
        });
      });

      Node::new(segment)
    })
  }

  /// Builds the child in `cx`.
  pub fn build(self, cx: &mut Cx<R>) -> Node {
    (self.0)(cx)
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
  pub fn set(&self, id: NodeId) {
    self.0.set(Some(id));
  }
}

/// A view whose type is erased.
#[must_use]
pub struct AnyView<R: Realization>(Child<R>);

impl<R: Realization> AnyView<R> {
  /// `view`, its type erased.
  pub fn new(view: impl View<R>) -> Self {
    Self(Child::view(view))
  }
}

impl<R: Realization> AnyKind for AnyView<R> {}

impl<R: Realization> View<R> for AnyView<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    self.0.build(cx)
  }

  fn into_any(self) -> Self {
    self
  }
}

/// Its view where there is one, and nothing where there is none: a node that takes no room
/// and no spacing in its parent's layout.
impl<V: AnyKind> AnyKind for Option<V> {}

impl<R: Realization, V: View<R>> View<R> for Option<V> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    match self {
      Some(view) => view.build(cx),
      None => Node::new(cx.create(NodeSpec::new(NodeKind::Segment))),
    }
  }
}

/// The view a closure returns, rebuilt in place whenever a signal the closure read changes;
/// what the previous one created is disposed with it.
///
/// It mounts under an anchor of its own, so its view joins its parent's layout where the
/// derived view stands.
#[must_use]
pub struct Derived<R: Realization> {
  build: Box<dyn Fn() -> AnyView<R>>,
}

wraps_itself!(Derived<R> where R: Realization);

/// What a [`Derived`] view carries: its constructor.
pub trait DerivedBase<R: Realization>: Wraps<Inner = Derived<R>> {
  /// The view `build` returns, kept current.
  fn new<V: View<R>>(build: impl Fn() -> V + 'static) -> Self {
    Self::wrap(Derived {
      build: Box::new(move || build().into_any()),
    })
  }
}

impl<R: Realization> DerivedBase<R> for Derived<R> {}

impl<R: Realization> AnyKind for Derived<R> {}

impl<R: Realization> View<R> for Derived<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let segment = cx.create(NodeSpec::new(NodeKind::Segment));
    let inner = cx.inherit();
    let shown = Cell::new(None::<NodeId>);
    let build = self.build;

    Effect::new(move || {
      let view = build();

      untrack(|| {
        let node = view.build(&mut inner.inherit());

        tree::attach::<R>(segment, node.id(), 0);

        if let Some(old) = shown.replace(Some(node.id())) {
          tree::remove::<R>(old);
        }
      });
    });

    Node::new(segment)
  }
}

/// Applies an optional setting: `.apply(change, Toggle::handle_change)` calls the setter where
/// there is a value and leaves the view as it is where there is none.
pub trait Apply: Sized {
  /// `set(self, value)` where `value` is `Some`, `self` otherwise.
  fn apply<T>(self, value: Option<T>, set: impl FnOnce(Self, T) -> Self) -> Self {
    match value {
      Some(value) => set(self, value),
      None => self,
    }
  }
}

impl<T: Wraps> Apply for T {}
