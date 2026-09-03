//! The box every layout view and the container are built from.

use std::rc::Rc;

use skene_core::{
  Bubbling,
  ClickHandler,
  Layout,
  Prevent,
  Role,
  next::{
    AnyKind,
    BoxDynamics,
    BoxProps,
    Children,
    ContainerName,
    DeclaredBoxStyle,
    Driver,
    HasChildren,
    NodeId,
    NodeKind,
    PlainChild,
    Scene,
    Seal,
    View,
  },
};

use crate::realize::bind_box_slot;

/// A box with a layout mode, a declared style, a role, a container name, click
/// handlers and children of kind `Kind`.
///
/// The one build every box shares: a view over it sets the properties its vocabulary
/// exposes into [`style_mut`](Self::style_mut) and delegates its children; a third
/// party's layout view does the same.
#[must_use]
pub struct LayoutBox<D: Driver, Kind = PlainChild> {
  layout: Layout,
  role: Option<Role>,
  container: Option<ContainerName>,
  style: DeclaredBoxStyle<D>,
  click: Vec<Rc<ClickHandler>>,
  children: Children<D, Kind>,
}

impl<D: Driver, Kind: 'static> LayoutBox<D, Kind> {
  /// A box laid out in `layout`, with no properties set.
  #[inline(always)]
  pub fn new(layout: Layout) -> Self {
    Self {
      layout,
      role: None,
      container: None,
      style: DeclaredBoxStyle::<D>::default(),
      click: Vec::new(),
      children: Children::new(),
    }
  }

  /// The declared style, for a view to write its properties into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<D> {
    &mut self.style
  }

  /// Sets the semantic role, guiding the driver's concrete choice of element.
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

  /// Runs `action` on a click; its verdict says whether the click carries on to the
  /// ancestors.
  #[inline(always)]
  pub fn handle_click<R: Into<Bubbling>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.click.push(Rc::new(move || {
      let bubbling: Bubbling = action().into();

      Prevent::from(bubbling)
    }));

    self
  }
}

impl<D: Driver, Kind: 'static> HasChildren<D> for LayoutBox<D, Kind> {
  type Kind = Kind;

  #[inline(always)]
  fn children_mut(&mut self, _: Seal) -> &mut Children<D, Kind> {
    &mut self.children
  }
}

impl<D: Driver, Kind: 'static> AnyKind for LayoutBox<D, Kind> {}

impl<D: Driver, Kind: 'static> View<D> for LayoutBox<D, Kind> {
  fn build(mut self) -> NodeId {
    if let Some(annotation) = Scene::<D>::with(Scene::take_pending_annotation) {
      annotation.apply_to(&mut self.style);
    }

    let (style, slots) = self.style.split();
    let props = BoxProps {
      layout: self.layout,
      role: self.role,
      style,
      dynamics: BoxDynamics::default(),
      container: self.container,
      focus_order: None,
      access: Vec::new(),
    };
    let driver = D::default();
    let sink = driver.create_box(&props);
    let container = self.container;
    let node = Scene::<D>::with(|scene| {
      let node = scene.create(NodeKind::Box, sink);

      if let Some(name) = container {
        scene.set_container(node, name);
      }

      node
    });

    for slot in slots {
      bind_box_slot::<D>(node, slot);
    }

    for handler in self.click {
      Scene::<D>::with(|scene| scene.handle_click(node, handler));
    }

    self.children.mount(node);

    node
  }
}
