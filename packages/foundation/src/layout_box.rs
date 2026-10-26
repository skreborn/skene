//! The box every layout view and the container are built from.

use std::rc::Rc;

use skene_reactive::Effect;

use crate::{
  AccessProp,
  Animation,
  AnyKind,
  Arrangement,
  BoxDynamics,
  BoxFacts,
  BoxKeyframe,
  Bubbling,
  Children,
  ClickEvent,
  CompiledStyle,
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
  NodeFacts,
  NodeHandle,
  NodeId,
  NodeSpec,
  PlainChild,
  PointerEvent,
  Prevent,
  Realization,
  Record,
  Role,
  Scene,
  ScrollEvent,
  Seal,
  Shortcut,
  Spacing,
  StyleExtension,
  Transition,
  View,
  WheelEvent,
  Wrap,
  contents,
  properties,
  realize::{Source, apply_style, bind_slot, report_access},
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
pub struct LayoutBox<P: Realization, Kind = PlainChild> {
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
  destination: Option<Source<Rc<str>>>,
  shortcut: Option<Shortcut>,
  drag_axes: Option<DragAxes>,
  handlers: Vec<Handler>,
  handles: Vec<NodeHandle<P>>,
  children: Children<P, Kind>,
}

impl<P: Realization, Kind> std::fmt::Debug for LayoutBox<P, Kind> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("LayoutBox").finish_non_exhaustive()
  }
}

impl<P: Realization, Kind: 'static> LayoutBox<P, Kind> {
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
      shortcut: None,
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

  /// This box over `defaults`: every property and fact this box sets in place of the
  /// default, its style overlaid on the defaults' field by field, and its motion,
  /// handlers and bound handles after the defaults'; its layout is its own, and its
  /// children are the defaults' where it has none.
  pub fn over(self, defaults: Self) -> Self {
    let mut style = defaults.style;
    let mut dynamics = defaults.dynamics;
    let mut handlers = defaults.handlers;
    let mut bound = defaults.handles;

    style.overlay(&self.style);
    dynamics.transitions.extend(self.dynamics.transitions);
    dynamics.animations.extend(self.dynamics.animations);
    handlers.extend(self.handlers);
    bound.extend(self.handles);

    Self {
      layout: self.layout,
      role: self.role.or(defaults.role),
      container: self.container.or(defaults.container),
      focus_order: self.focus_order.or(defaults.focus_order),
      access: self.access.or(defaults.access),
      label: self.label.or(defaults.label),
      labelled_by: self.labelled_by.or(defaults.labelled_by),
      style,
      dynamics,
      transition_name: self.transition_name.or(defaults.transition_name),
      destination: self.destination.or(defaults.destination),
      shortcut: self.shortcut.or(defaults.shortcut),
      drag_axes: self.drag_axes.or(defaults.drag_axes),
      handlers,
      handles: bound,
      children: if self.children.is_empty() {
        defaults.children
      }
      else {
        self.children
      },
    }
  }

  properties! { style_mut, P;
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

  crate::alignment_properties! { style_mut, P;
    set_alignment / derive_alignment, "the alignment of each child",
    "The horizontal component lands on the inline axis, the vertical one on the cross or \
     block axis."
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
  pub fn add_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.dynamics.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn add_animation(mut self, animation: Animation<BoxKeyframe<P>>) -> Self {
    self.dynamics.animations.push(animation);

    self
  }

  /// Names the box for view transitions: the box is paired with the box of the same
  /// name across a navigation, where the platform animates between them.
  pub fn set_transition_name(mut self, name: impl Into<String>) -> Self {
    self.transition_name = Some(name.into());

    self
  }

  /// Sets the path the box navigates to when activated, making it a link: a tab stop
  /// unless its focus order says otherwise, realized as a real link where the medium
  /// has one. A destination that would run script when followed, a `javascript:` URL,
  /// is never followed.
  pub fn set_destination(mut self, path: impl Into<String>) -> Self {
    self.destination = Some(Source::Fixed(Rc::from(path.into())));

    self
  }

  /// Derives the path the box navigates to when activated from the signals `path`
  /// reads, making it a link as [`set_destination`](Self::set_destination) does.
  pub fn derive_destination(mut self, path: impl Fn() -> String + 'static) -> Self {
    self.destination = Some(Source::derive(move || Rc::from(path())));

    self
  }

  /// Runs `action` on a click; its verdict says whether the click carries on to the
  /// ancestors.
  ///
  /// A box that handles a click and is no link is an activatable row: a tab stop
  /// unless its focus order says otherwise, `Enter` and `Space` activating it as a click
  /// does while it holds focus, and the controls inside it owning their presses.
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self
      .handlers
      .push(Handler::Click(Rc::new(move |_: &ClickEvent| {
        verdict(action())
      })));

    self
  }

  /// Sets the key that activates the box from anywhere in the window, as a click does,
  /// where the box is an activatable row; a box handling no click ignores it.
  #[inline(always)]
  pub const fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.shortcut = Some(shortcut);

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
  /// the box holds [`State::Dragged`](crate::State::Dragged) while the drag lasts.
  pub fn set_drag_data(mut self, data: impl Fn() -> DragData + 'static) -> Self {
    self.handlers.push(Handler::DragStart(Rc::new(data)));

    self
  }

  /// Takes a drag carrying a `T` dropped on the box, running `action` with the value;
  /// the box holds [`State::Targeted`](crate::State::Targeted) while such a drag
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
fn verdict<R: Into<Bubbling>>(result: R) -> Prevent {
  let bubbling: Bubbling = result.into();

  Prevent::from(bubbling)
}

impl<P: Realization, Kind: 'static> HasChildren<P> for LayoutBox<P, Kind> {
  type Kind = Kind;

  #[inline(always)]
  fn children_mut(&mut self, _: Seal) -> &mut Children<P, Kind> {
    &mut self.children
  }
}

impl<P: Realization, Kind: 'static> AnyKind for LayoutBox<P, Kind> {}

impl<P: Realization, Kind: 'static> LayoutBox<P, Kind> {
  /// Folds the box's name, its label and the node labelling it, into its accessibility,
  /// and its shortcut where `acting`, an activatable row's.
  fn fold_name(&mut self, acting: bool) {
    let named: Vec<AccessProp> = self
      .label
      .take()
      .map(AccessProp::Name)
      .into_iter()
      .chain(
        self
          .labelled_by
          .map(|node| AccessProp::LabelledBy(vec![node])),
      )
      .chain(self.shortcut.filter(|_| acting).map(AccessProp::Shortcut))
      .collect();

    if named.is_empty() {
      return;
    }

    self.access = Some(match self.access.take() {
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

  /// Realizes the box as the node whose facts `facts` makes of the box's: folds the
  /// pending annotation into the style, splits it, creates the node on the platform
  /// from its [`NodeSpec`], calls `realized` with the node, its spec and its style, then
  /// applies the style, writes its fixed focus order, a tab stop where it is an
  /// activatable row, and its accessibility, records its shortcut where it is one,
  /// binds every derived slot, a derived focus order and accessibility, and the
  /// handlers, mounts the children and returns the node.
  ///
  /// A box builds through this with [`NodeFacts::Box`]; a view realized as another kind
  /// makes its facts from the box's.
  pub fn realize(
    mut self,
    facts: impl FnOnce(BoxFacts) -> NodeFacts,
    realized: impl FnOnce(NodeId, &NodeSpec, &CompiledStyle<P::Scalar, P::StyleExt>),
  ) -> NodeId {
    if let Some(annotation) = Scene::<P>::with(Scene::take_pending_annotation) {
      annotation.apply_to(&mut self.style);
    }

    let links = self.destination.is_some();
    let acting = !links
      && self
        .handlers
        .iter()
        .any(|handler| matches!(handler, Handler::Click(_)));
    let facts = facts(BoxFacts {
      layout: self.layout,
      container: self.container,
      transition_name: self.transition_name.take(),
      link: links,
      acting,
    });
    let acting = matches!(&facts, NodeFacts::Box(facts) if facts.acting);

    self.fold_name(acting);

    let (style, slots) = (*self.style).split();
    let mut record = Record::new();

    record.add_box(&style);
    record.add_box_dynamics(&self.dynamics);

    let style = record.into_kept();
    let focus_order = match &self.focus_order {
      Some(Source::Fixed(focus_order)) => Some(*focus_order),
      Some(Source::Derived(_)) | None => None,
    };
    let noted = match &self.access {
      Some(Source::Fixed(access)) => Some(AccessProp::fold(access.iter().cloned())),
      Some(Source::Derived(_)) | None => None,
    };
    let access = noted.clone().unwrap_or_default();
    let spec = NodeSpec::new(
      self.role,
      AccessProp::level_of(&access),
      self.drag_axes,
      facts,
    );
    let tab_stop = focus_order.or_else(|| spec.acting().then_some(FocusOrder::Auto));
    let shortcut = self.shortcut.filter(|_| acting);
    let container = self.container;
    let node = Scene::<P>::realize(&spec);

    if let Some(destination) = self.destination {
      destination.bind(node, |node, path| {
        contents::DESTINATION.write::<P>(node, path);
      });
    }

    realized(node, &spec, &style);
    apply_style::<P>(node, style);
    P::update_focus_order(node, tab_stop);
    P::update_access(node, &access);
    Scene::<P>::with(|scene| {
      scene.note_focus_order(node, tab_stop.or_else(|| links.then_some(FocusOrder::Auto)));

      if let Some(name) = container {
        scene.set_container(node, name);
      }

      if let Some(access) = noted {
        scene.note_access(node, access);
      }

      if let Some(shortcut) = shortcut {
        scene.note_shortcut(node, shortcut);
      }
    });

    for handle in self.handles {
      handle.assign(node);
    }

    for slot in slots {
      bind_slot::<P, _, _>(node, slot);
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

impl<P: Realization, Kind: 'static> View<P> for LayoutBox<P, Kind> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.realize(NodeFacts::Box, |_, _, _| {})
  }
}
