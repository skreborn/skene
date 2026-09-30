//! The box every layout view and the container are built from.

use std::rc::Rc;

use skene_core::{
  AccessProp,
  Alignment,
  Animation,
  AnyKind,
  Arrangement,
  BoxDynamics,
  BoxKeyframe,
  BoxProps,
  Bubbling,
  Children,
  ClickEvent,
  Condition,
  ContainerName,
  DeclaredBoxStyle,
  DragAxes,
  DragData,
  DropPhase,
  FocusOrder,
  GridFlow,
  GridTemplate,
  GridTrack,
  Handler,
  HasChildren,
  Inset,
  ItemAlignment,
  KeyEvent,
  Layout,
  Leaf,
  NodeId,
  NodeKind,
  PlainChild,
  Platform,
  PointerEvent,
  Prevent,
  Role,
  Scene,
  ScrollEvent,
  Seal,
  Spacing,
  StaticLeaf,
  StyleExtension,
  Transition,
  Value,
  View,
  WheelEvent,
  Wrap,
};
use skene_reactive::Effect;

use crate::{
  NodeHandle,
  properties,
  realize::{Source, bind_box_slot, report_access},
};

/// A box with a layout mode, a declared style, a role, a container name, focus and
/// accessibility, motion, handlers and children of kind `Kind`.
///
/// The one build every box shares, carrying the whole layout vocabulary: a view over
/// it exposes the properties its layout honors and delegates its children, and a
/// third party's layout view does the same. Decoration and sizing are set through
/// [`style_mut`](Self::style_mut) by the one view that offers them,
/// [`Container`](crate::Container). A [`NodeHandle`] bound to the box names its node
/// once built.
#[must_use]
pub struct LayoutBox<P: Platform, Kind = PlainChild> {
  layout: Layout,
  role: Option<Role>,
  container: Option<ContainerName>,
  focus_order: Option<Source<FocusOrder>>,
  access: Option<Source<Vec<AccessProp>>>,
  label: Option<String>,
  labelled_by: Option<NodeId>,
  style: Box<DeclaredBoxStyle<P>>,
  dynamics: BoxDynamics<P::Scalar, P::StyleExt>,
  transition_name: Option<String>,
  destination: Option<String>,
  drag_axes: Option<DragAxes>,
  handlers: Vec<Handler>,
  handles: Vec<NodeHandle<P>>,
  children: Children<P, Kind>,
}

impl<P: Platform, Kind> std::fmt::Debug for LayoutBox<P, Kind> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("LayoutBox").finish_non_exhaustive()
  }
}

impl<P: Platform, Kind: 'static> LayoutBox<P, Kind> {
  /// A box laid out in `layout`, with no properties set.
  #[inline(always)]
  pub fn new(layout: Layout) -> Self {
    Self {
      layout,
      role: None,
      container: None,
      focus_order: None,
      access: None,
      label: None,
      labelled_by: None,
      style: Box::default(),
      dynamics: BoxDynamics::default(),
      transition_name: None,
      destination: None,
      drag_axes: None,
      handlers: Vec::new(),
      handles: Vec::new(),
      children: Children::new(),
    }
  }

  /// Sets the layout the box lays its children out in.
  #[inline(always)]
  pub const fn set_layout(mut self, layout: Layout) -> Self {
    self.layout = layout;

    self
  }

  /// The declared style, for a view to write its properties into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    &mut self.style
  }

  properties! { style_mut;
    set_padding / derive_padding (padding) => padding: Inset<P::Scalar>,
      "the inset between the box and the children it arranges";
    set_spacing / derive_spacing (spacing) => spacing: Spacing<P::Scalar>,
      "the spacing between children, per axis";
    set_arrangement / derive_arrangement (arrangement) => arrangement: Arrangement,
      "the distribution of children along the main or inline axis";
    set_item_alignment / derive_item_alignment (alignment) => item_alignment: ItemAlignment,
      "the alignment of each child on the cross or block axis";
    set_line_arrangement / derive_line_arrangement (arrangement) => line_arrangement: Arrangement,
      "the distribution of wrapped lines or grid tracks along the cross or block axis";
    set_item_horizontal_alignment / derive_item_horizontal_alignment (alignment)
      => item_horizontal_alignment: ItemAlignment,
      "the alignment of each grid child on the inline axis";
    set_wrap / derive_wrap (wrap) => wrap: Wrap,
      "whether and how children wrap onto further lines";
    set_columns / derive_columns (tracks) => grid_columns: GridTemplate<P::Scalar>,
      "the explicit column tracks, with the names of their lines; a list of tracks names none";
    set_rows / derive_rows (tracks) => grid_rows: GridTemplate<P::Scalar>,
      "the explicit row tracks, with the names of their lines; a list of tracks names none";
    set_implicit_columns / derive_implicit_columns (tracks)
      => grid_implicit_columns: Vec<GridTrack<P::Scalar>>,
      "the sizes given to implicitly created columns, cycled";
    set_implicit_rows / derive_implicit_rows (tracks)
      => grid_implicit_rows: Vec<GridTrack<P::Scalar>>,
      "the sizes given to implicitly created rows, cycled";
    set_flow / derive_flow (flow) => grid_flow: GridFlow,
      "the direction auto-placed children fill";
  }

  /// Sets the alignment of each child on both axes: the horizontal component lands on
  /// the inline axis, the vertical one on the cross or block axis.
  pub fn set_alignment(
    mut self,
    alignment: impl Into<Value<Alignment, Condition<P::Scalar>>>,
  ) -> Self {
    let (horizontal, vertical) = split_alignment(&alignment.into());

    self.style.item_horizontal_alignment = Some(horizontal);
    self.style.item_alignment = Some(vertical);

    self
  }

  /// Derives the alignment of each child on both axes from the signals `alignment`
  /// reads: a value, a token or an expression.
  pub fn derive_alignment<R: Into<StaticLeaf<Alignment>>>(
    self,
    alignment: impl Fn() -> R + 'static,
  ) -> Self {
    self.set_alignment(Value::<Alignment, Condition<P::Scalar>>::new(Leaf::derive(
      alignment,
    )))
  }

  /// Sets the semantic role, guiding the platform's concrete choice of element.
  #[inline(always)]
  pub const fn set_role(mut self, role: Role) -> Self {
    self.role = Some(role);

    self
  }

  /// Names the box for container queries against its size.
  #[inline(always)]
  pub const fn set_container_name(mut self, name: ContainerName) -> Self {
    self.container = Some(name);

    self
  }

  /// Declares the axes a press's pointer drags the box along: the press's moves and
  /// its release reach the box wherever the pointer goes, and a touch pans only along
  /// the others.
  #[inline(always)]
  pub const fn set_drag_axes(mut self, axes: DragAxes) -> Self {
    self.drag_axes = Some(axes);

    self
  }

  /// Sets the box's place in sequential focus traversal; a box without one takes no
  /// focus.
  #[inline(always)]
  pub fn set_focus_order(mut self, focus_order: FocusOrder) -> Self {
    self.focus_order = Some(Source::Fixed(focus_order));

    self
  }

  /// Derives the box's place in sequential focus traversal from the signals
  /// `focus_order` reads.
  pub fn derive_focus_order(mut self, focus_order: impl Fn() -> FocusOrder + 'static) -> Self {
    self.focus_order = Some(Source::derive(focus_order));

    self
  }

  /// Sets what the box reports to assistive technology, beside its role.
  pub fn set_accessibility(mut self, access: impl IntoIterator<Item = AccessProp>) -> Self {
    self.access = Some(Source::Fixed(access.into_iter().collect()));

    self
  }

  /// Sets the box's accessible name, for a region, a dialog or a picture that no visible
  /// text names.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets the node whose text names the box: a dialog's heading, a region's title.
  #[inline(always)]
  pub const fn set_labelled_by(mut self, node: NodeId) -> Self {
    self.labelled_by = Some(node);

    self
  }

  /// Derives what the box reports to assistive technology from the signals `access`
  /// reads.
  pub fn derive_accessibility(mut self, access: impl Fn() -> Vec<AccessProp> + 'static) -> Self {
    self.access = Some(Source::derive(access));

    self
  }

  /// Binds `handle` to the box once built: an overlay is placed against it, focus
  /// moves to it and its measured size is reported through the handle.
  #[inline(always)]
  pub fn bind_handle(mut self, handle: NodeHandle<P>) -> Self {
    self.handles.push(handle);

    self
  }

  /// Adds a property transition, interpolating that property whenever its value
  /// changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.dynamics.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<BoxKeyframe<P>>) -> Self {
    self.dynamics.animations.push(animation);

    self
  }

  /// Names the box for view transitions: the box is paired with the box of the same
  /// name across a navigation, where the platform animates between them.
  pub fn set_transition_name(mut self, name: impl Into<String>) -> Self {
    self.transition_name = Some(name.into());

    self
  }

  /// Sets the path the box navigates to when activated; the medium realizes it as a
  /// real link where it has one. A destination that would run script when followed, a
  /// `javascript:` URL, is never followed.
  pub fn set_destination(mut self, path: impl Into<String>) -> Self {
    self.destination = Some(path.into());

    self
  }

  /// Runs `action` on a click; its verdict says whether the click carries on to the
  /// ancestors.
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self
      .handlers
      .push(Handler::Click(Rc::new(move |_: &ClickEvent| {
        verdict(action())
      })));

    self
  }

  /// Runs `action` on the second click of a double click, after the click itself; its
  /// verdict says whether the double click carries on to the ancestors.
  pub fn handle_double_click<R: Into<Bubbling>>(
    mut self,
    action: impl Fn() -> R + 'static,
  ) -> Self {
    self
      .handlers
      .push(Handler::DoubleClick(Rc::new(move |_: &ClickEvent| {
        verdict(action())
      })));

    self
  }

  /// Runs `action` on a key pressed while the box or a descendant holds focus; its
  /// verdict says whether the key carries on to the ancestors.
  pub fn handle_key_down<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.handlers.push(Handler::KeyDown(Rc::new(move |event| {
      verdict(action(event))
    })));

    self
  }

  /// Runs `action` on a key released while the box or a descendant holds focus; its
  /// verdict says whether the release carries on to the ancestors.
  pub fn handle_key_up<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self
      .handlers
      .push(Handler::KeyUp(Rc::new(move |event| verdict(action(event)))));

    self
  }

  /// Runs `action` on a pointer button pressed over the box; its verdict says whether
  /// the press carries on to the ancestors.
  pub fn handle_pointer_down<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self
      .handlers
      .push(Handler::PointerDown(Rc::new(move |event| {
        verdict(action(event))
      })));

    self
  }

  /// Runs `action` on a pointer button released over the box; its verdict says whether
  /// the release carries on to the ancestors.
  pub fn handle_pointer_up<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.handlers.push(Handler::PointerUp(Rc::new(move |event| {
      verdict(action(event))
    })));

    self
  }

  /// Runs `action` on the pointer moving over the box; its verdict says whether the
  /// move carries on to the ancestors.
  pub fn handle_pointer_move<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self
      .handlers
      .push(Handler::PointerMove(Rc::new(move |event| {
        verdict(action(event))
      })));

    self
  }

  /// Offers what a drag from the box carries, `data` building it as the drag starts;
  /// the box holds [`State::Dragged`](skene_core::State::Dragged) while the drag lasts.
  pub fn set_drag_data(mut self, data: impl Fn() -> DragData + 'static) -> Self {
    self.handlers.push(Handler::DragStart(Rc::new(data)));

    self
  }

  /// Takes a drag carrying a `T` dropped on the box, running `action` with the value;
  /// the box holds [`State::Targeted`](skene_core::State::Targeted) while such a drag
  /// is over it.
  pub fn handle_drop<T: 'static>(mut self, action: impl Fn(&T) + 'static) -> Self {
    self
      .handlers
      .push(Handler::Drop(Rc::new(move |data: &DragData, phase| {
        let Some(value) = data.value::<T>()
        else {
          return false;
        };

        if phase == DropPhase::Dropped {
          action(value);
        }

        true
      })));

    self
  }

  /// Runs `action` on a wheel turned over the box, before the platform scrolls by it;
  /// its verdict says whether the turn carries on to the ancestors.
  pub fn handle_wheel<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&WheelEvent) -> R + 'static,
  ) -> Self {
    self
      .handlers
      .push(Handler::Wheel(Rc::new(move |event| verdict(action(event)))));

    self
  }

  /// Runs `action` when the platform takes a pointer pressed over the box for itself,
  /// ending the press without a release; its verdict says whether the cancel carries on
  /// to the ancestors.
  pub fn handle_pointer_cancel<R: Into<Bubbling>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self
      .handlers
      .push(Handler::PointerCancel(Rc::new(move |event| {
        verdict(action(event))
      })));

    self
  }

  /// Runs `action` when the pointer enters the box.
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.handlers.push(Handler::PointerEnter(Rc::new(action)));

    self
  }

  /// Runs `action` when the pointer leaves the box.
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.handlers.push(Handler::PointerLeave(Rc::new(action)));

    self
  }

  /// Runs `action` when focus enters the box or a node inside it from outside.
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.handlers.push(Handler::Focus(Rc::new(action)));

    self
  }

  /// Runs `action` when focus leaves the box and every node inside it.
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.handlers.push(Handler::Blur(Rc::new(action)));

    self
  }

  /// Runs `action` with the box's new offsets when it scrolls.
  pub fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.handlers.push(Handler::Scroll(Rc::new(action)));

    self
  }
}

/// A handler's verdict as dispatch stores it.
pub(crate) fn verdict<R: Into<Bubbling>>(result: R) -> Prevent {
  let bubbling: Bubbling = result.into();

  Prevent::from(bubbling)
}

/// One axis of a two-axis alignment leaf: a fixed alignment projected through `pick`, a
/// token read by its name.
fn alignment_axis_leaf(
  leaf: &StaticLeaf<Alignment>,
  pick: fn(Alignment) -> ItemAlignment,
) -> StaticLeaf<ItemAlignment> {
  match leaf {
    StaticLeaf::Fixed(alignment) => StaticLeaf::Fixed(pick(*alignment)),
    StaticLeaf::Token(name) => StaticLeaf::Token(*name),
    StaticLeaf::Expr(_) => unreachable!("no expression has an alignment type"),
  }
}

/// One axis of a two-axis alignment value: each leaf projected through `pick`.
fn alignment_axis<C: Clone>(
  value: &Value<Alignment, C>,
  pick: fn(Alignment) -> ItemAlignment,
) -> Value<ItemAlignment, C> {
  value.clone().map(|leaf| match leaf {
    Leaf::Static(leaf) => Leaf::Static(alignment_axis_leaf(&leaf, pick)),
    Leaf::Derived(compute) => Leaf::Derived(Rc::new(move || alignment_axis_leaf(&compute(), pick))),
  })
}

/// Splits a two-axis alignment value into its horizontal and vertical axes.
pub(crate) fn split_alignment<C: Clone>(
  value: &Value<Alignment, C>,
) -> (Value<ItemAlignment, C>, Value<ItemAlignment, C>) {
  (
    alignment_axis(value, |alignment| alignment.horizontal),
    alignment_axis(value, |alignment| alignment.vertical),
  )
}

impl<P: Platform, Kind: 'static> HasChildren<P> for LayoutBox<P, Kind> {
  type Kind = Kind;

  #[inline(always)]
  fn children_mut(&mut self, _: Seal) -> &mut Children<P, Kind> {
    &mut self.children
  }
}

impl<P: Platform, Kind: 'static> AnyKind for LayoutBox<P, Kind> {}

impl<P: Platform, Kind: 'static> LayoutBox<P, Kind> {
  /// Realizes the box as a node of `kind`, which `create` realizes on the platform from
  /// the props: folds the pending annotation into the style, splits it, binds every
  /// derived slot, the focus order, the accessibility and the handlers, mounts the
  /// children and returns the node.
  ///
  /// A box builds through this with [`NodeKind::Box`] and [`Platform::create_box`]; a
  /// view realized as another kind supplies its own creation.
  pub fn realize(mut self, kind: NodeKind, create: impl FnOnce(NodeId, BoxProps<P>)) -> NodeId {
    if let Some(annotation) = Scene::<P>::with(Scene::take_pending_annotation) {
      annotation.apply_to(&mut self.style);
    }

    let named: Vec<AccessProp> = self
      .label
      .map(AccessProp::Name)
      .into_iter()
      .chain(
        self
          .labelled_by
          .map(|node| AccessProp::LabelledBy(vec![node])),
      )
      .collect();

    if !named.is_empty() {
      self.access = Some(match self.access {
        None => Source::Fixed(named),
        Some(Source::Fixed(mut access)) => {
          access.extend(named);

          Source::Fixed(access)
        }
        Some(Source::Derived(derive)) => Source::derive(move || {
          let mut access = derive();

          access.extend(named.iter().cloned());

          access
        }),
      });
    }

    let (style, slots) = (*self.style).split();
    let focus_order = match &self.focus_order {
      Some(Source::Fixed(focus_order)) => Some(*focus_order),
      Some(Source::Derived(_)) | None => None,
    };
    let noted = match &self.access {
      Some(Source::Fixed(access)) => Some(AccessProp::fold(access.iter().cloned())),
      Some(Source::Derived(_)) | None => None,
    };
    let access = noted.clone().unwrap_or_default();
    let props = BoxProps {
      layout: self.layout,
      role: self.role,
      style,
      dynamics: self.dynamics,
      transition_name: self.transition_name,
      container: self.container,
      focus_order,
      drag_axes: self.drag_axes,
      access,
      destination: self.destination,
    };
    let container = self.container;
    let node = Scene::<P>::with(|scene| scene.create(kind, None));

    create(node, props);
    Scene::<P>::with(|scene| {
      scene.note_focus_order(node, focus_order);

      if let Some(name) = container {
        scene.set_container(node, name);
      }

      if let Some(access) = noted {
        scene.note_access(node, access);
      }
    });

    for handle in self.handles {
      handle.assign(node);
    }

    for slot in slots {
      bind_box_slot::<P>(node, slot);
    }

    if let Some(Source::Derived(focus_order)) = self.focus_order {
      Effect::render(move |_: Option<()>| {
        let focus_order = focus_order();

        Scene::<P>::with(|scene| scene.set_focus_order(node, Some(focus_order)));
      });
    }

    if let Some(Source::Derived(access)) = self.access {
      Effect::render(move |_: Option<()>| report_access::<P>(node, access()));
    }

    Scene::<P>::with(|scene| {
      for handler in self.handlers {
        scene.handle(node, handler);
      }
    });

    self.children.mount(node);

    node
  }
}

impl<P: Platform, Kind: 'static> View<P> for LayoutBox<P, Kind> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.realize(NodeKind::Box, |node, props| P::create_box(node, &props))
  }
}
