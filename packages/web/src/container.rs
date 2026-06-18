use std::rc::Rc;

use skene_core::{
  Children,
  Color,
  CrossAxisAlignment,
  HasChildren,
  Inset,
  MainAxisAlignment,
  MouseCursor,
  NodeId,
  Number,
  Source,
  View,
};

use crate::{Border, Declarations, Role, StyleValue, styler::Styler, with_backend};

/// A box that decorates itself and lays out its children.
///
/// Constructed in a layout mode: [`new`](Container::new) (block normal flow),
/// [`row`](Container::row) / [`column`](Container::column) (flex),
/// [`grid`](Container::grid), or [`stack`](Container::stack) (children overlapping in
/// one cell). Children are set with [`set_children`](HasChildren::set_children) or
/// [`derive_children`](HasChildren::derive_children); decoration, sizing, and layout
/// are set with the chainable `set_`/`derive_` methods.
#[must_use]
pub struct Container {
  children: Children,
  base: Declarations,
  stack: bool,
  role: Option<Role>,
  width: Option<Source<Number>>,
  height: Option<Source<Number>>,
  min_width: Option<Source<Number>>,
  min_height: Option<Source<Number>>,
  max_width: Option<Source<Number>>,
  max_height: Option<Source<Number>>,
  padding: Option<Source<Inset>>,
  color: Option<Source<Color>>,
  border: Option<Source<Border>>,
  border_radius: Option<Source<Number>>,
  cursor: Option<Source<MouseCursor>>,
  spacing: Option<Source<Number>>,
  main_axis_alignment: Option<Source<MainAxisAlignment>>,
  cross_axis_alignment: Option<Source<CrossAxisAlignment>>,
  hooks: Vec<Box<dyn FnOnce(NodeId)>>,
}

impl Container {
  fn mode(base: Declarations, stack: bool) -> Self {
    Self {
      children: Children::default(),
      base,
      stack,
      role: None,
      width: None,
      height: None,
      min_width: None,
      min_height: None,
      max_width: None,
      max_height: None,
      padding: None,
      color: None,
      border: None,
      border_radius: None,
      cursor: None,
      spacing: None,
      main_axis_alignment: None,
      cross_axis_alignment: None,
      hooks: Vec::new(),
    }
  }

  /// A block container that establishes an independent formatting context: children
  /// flow in normal document order, contained within this box.
  #[inline(always)]
  pub fn new() -> Self {
    Container::mode(
      Declarations::new().set("display", StyleValue::Keyword("flow-root")),
      false,
    )
  }

  /// A flex container laying children out left to right.
  #[inline(always)]
  pub fn row() -> Self {
    Container::mode(
      Declarations::new()
        .set("display", StyleValue::Keyword("flex"))
        .set("flex-direction", StyleValue::Keyword("row")),
      false,
    )
  }

  /// A flex container laying children out top to bottom.
  #[inline(always)]
  pub fn column() -> Self {
    Container::mode(
      Declarations::new()
        .set("display", StyleValue::Keyword("flex"))
        .set("flex-direction", StyleValue::Keyword("column")),
      false,
    )
  }

  /// A grid container with `columns` equal-width columns.
  #[inline(always)]
  pub fn grid(columns: u16) -> Self {
    Container::mode(
      Declarations::new()
        .set("display", StyleValue::Keyword("grid"))
        .set(
          "grid-template-columns",
          StyleValue::Custom(format!("repeat({columns}, 1fr)")),
        ),
      false,
    )
  }

  /// A container overlapping its children in one cell, stacked in order (last on top).
  #[inline(always)]
  pub fn stack() -> Self {
    Container::mode(
      Declarations::new().set("display", StyleValue::Keyword("grid")),
      true,
    )
  }

  /// Sets the semantic role.
  #[inline(always)]
  pub fn set_role(mut self, role: Role) -> Self {
    self.role = Some(role);

    self
  }

  /// Sets a static width.
  #[inline(always)]
  pub fn set_width(mut self, width: impl Into<Number>) -> Self {
    self.width = Some(Source::Static(width.into()));

    self
  }

  /// Sets a derived width.
  #[inline(always)]
  pub fn derive_width(mut self, width: impl Fn() -> Number + 'static) -> Self {
    self.width = Some(Source::Derived(Box::new(width)));

    self
  }

  /// Sets a static height.
  #[inline(always)]
  pub fn set_height(mut self, height: impl Into<Number>) -> Self {
    self.height = Some(Source::Static(height.into()));

    self
  }

  /// Sets a derived height.
  #[inline(always)]
  pub fn derive_height(mut self, height: impl Fn() -> Number + 'static) -> Self {
    self.height = Some(Source::Derived(Box::new(height)));

    self
  }

  /// Sets a static minimum width.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: impl Into<Number>) -> Self {
    self.min_width = Some(Source::Static(min_width.into()));

    self
  }

  /// Sets a derived minimum width.
  #[inline(always)]
  pub fn derive_min_width(mut self, min_width: impl Fn() -> Number + 'static) -> Self {
    self.min_width = Some(Source::Derived(Box::new(min_width)));

    self
  }

  /// Sets a static minimum height.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: impl Into<Number>) -> Self {
    self.min_height = Some(Source::Static(min_height.into()));

    self
  }

  /// Sets a derived minimum height.
  #[inline(always)]
  pub fn derive_min_height(mut self, min_height: impl Fn() -> Number + 'static) -> Self {
    self.min_height = Some(Source::Derived(Box::new(min_height)));

    self
  }

  /// Sets a static maximum width.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: impl Into<Number>) -> Self {
    self.max_width = Some(Source::Static(max_width.into()));

    self
  }

  /// Sets a derived maximum width.
  #[inline(always)]
  pub fn derive_max_width(mut self, max_width: impl Fn() -> Number + 'static) -> Self {
    self.max_width = Some(Source::Derived(Box::new(max_width)));

    self
  }

  /// Sets a static maximum height.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: impl Into<Number>) -> Self {
    self.max_height = Some(Source::Static(max_height.into()));

    self
  }

  /// Sets a derived maximum height.
  #[inline(always)]
  pub fn derive_max_height(mut self, max_height: impl Fn() -> Number + 'static) -> Self {
    self.max_height = Some(Source::Derived(Box::new(max_height)));

    self
  }

  /// Sets a static padding.
  #[inline(always)]
  pub fn set_padding(mut self, padding: impl Into<Inset>) -> Self {
    self.padding = Some(Source::Static(padding.into()));

    self
  }

  /// Sets a derived padding.
  #[inline(always)]
  pub fn derive_padding(mut self, padding: impl Fn() -> Inset + 'static) -> Self {
    self.padding = Some(Source::Derived(Box::new(padding)));

    self
  }

  /// Sets a static background color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.color = Some(Source::Static(color));

    self
  }

  /// Sets a derived background color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.color = Some(Source::Derived(Box::new(color)));

    self
  }

  /// Sets a static border.
  #[inline(always)]
  pub fn set_border(mut self, border: Border) -> Self {
    self.border = Some(Source::Static(border));

    self
  }

  /// Sets a derived border.
  #[inline(always)]
  pub fn derive_border(mut self, border: impl Fn() -> Border + 'static) -> Self {
    self.border = Some(Source::Derived(Box::new(border)));

    self
  }

  /// Sets a static border radius.
  #[inline(always)]
  pub fn set_border_radius(mut self, border_radius: impl Into<Number>) -> Self {
    self.border_radius = Some(Source::Static(border_radius.into()));

    self
  }

  /// Sets a derived border radius.
  #[inline(always)]
  pub fn derive_border_radius(mut self, border_radius: impl Fn() -> Number + 'static) -> Self {
    self.border_radius = Some(Source::Derived(Box::new(border_radius)));

    self
  }

  /// Sets a static mouse cursor.
  #[inline(always)]
  pub fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.cursor = Some(Source::Static(cursor));

    self
  }

  /// Sets a derived mouse cursor.
  #[inline(always)]
  pub fn derive_cursor(mut self, cursor: impl Fn() -> MouseCursor + 'static) -> Self {
    self.cursor = Some(Source::Derived(Box::new(cursor)));

    self
  }

  /// Sets a static gap between children.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: impl Into<Number>) -> Self {
    self.spacing = Some(Source::Static(spacing.into()));

    self
  }

  /// Sets a derived gap between children.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> Number + 'static) -> Self {
    self.spacing = Some(Source::Derived(Box::new(spacing)));

    self
  }

  /// Sets a static main axis alignment.
  #[inline(always)]
  pub fn set_main_axis_alignment(mut self, alignment: MainAxisAlignment) -> Self {
    self.main_axis_alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived main axis alignment.
  #[inline(always)]
  pub fn derive_main_axis_alignment(
    mut self,
    alignment: impl Fn() -> MainAxisAlignment + 'static,
  ) -> Self {
    self.main_axis_alignment = Some(Source::Derived(Box::new(alignment)));

    self
  }

  /// Sets a static cross axis alignment.
  #[inline(always)]
  pub fn set_cross_axis_alignment(mut self, alignment: CrossAxisAlignment) -> Self {
    self.cross_axis_alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived cross axis alignment.
  #[inline(always)]
  pub fn derive_cross_axis_alignment(
    mut self,
    alignment: impl Fn() -> CrossAxisAlignment + 'static,
  ) -> Self {
    self.cross_axis_alignment = Some(Source::Derived(Box::new(alignment)));

    self
  }

  /// Sets the press handler.
  #[inline(always)]
  pub fn on_press(mut self, action: impl Fn() + 'static) -> Self {
    let action: Rc<dyn Fn()> = Rc::new(action);

    self.hooks.push(Box::new(move |node| {
      with_backend(|backend| backend.on_click(node, action));
    }));

    self
  }
}

impl Default for Container {
  #[inline(always)]
  fn default() -> Self {
    Container::new()
  }
}

impl HasChildren for Container {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for Container {
  fn build(self) -> NodeId {
    let node = with_backend(|backend| backend.create_container(self.role));

    Styler::new(node, self.base, |node, declarations| {
      with_backend(|backend| backend.set_css(node, &declarations));
    })
    .prop(self.width, |declarations, value| {
      declarations.set("width", StyleValue::Length(value))
    })
    .prop(self.height, |declarations, value| {
      declarations.set("height", StyleValue::Length(value))
    })
    .prop(self.min_width, |declarations, value| {
      declarations.set("min-width", StyleValue::Length(value))
    })
    .prop(self.min_height, |declarations, value| {
      declarations.set("min-height", StyleValue::Length(value))
    })
    .prop(self.max_width, |declarations, value| {
      declarations.set("max-width", StyleValue::Length(value))
    })
    .prop(self.max_height, |declarations, value| {
      declarations.set("max-height", StyleValue::Length(value))
    })
    .prop(self.color, |declarations, value| {
      declarations.set("background-color", StyleValue::Color(value))
    })
    .prop(self.border, |declarations, value: Border| {
      declarations
        .set("border-width", StyleValue::Length(value.width))
        .set("border-style", StyleValue::Keyword("solid"))
        .set("border-color", StyleValue::Color(value.color))
    })
    .prop(self.border_radius, |declarations, value| {
      declarations.set("border-radius", StyleValue::Length(value))
    })
    .prop(self.cursor, |declarations, value| {
      declarations.set(
        "cursor",
        StyleValue::Keyword(match value {
          MouseCursor::Default => "default",
          MouseCursor::Click => "pointer",
          MouseCursor::Text => "text",
        }),
      )
    })
    .prop(self.padding, |declarations, value| {
      declarations.set("padding", StyleValue::Padding(value))
    })
    .prop(self.spacing, |declarations, value| {
      declarations.set("gap", StyleValue::Length(value))
    })
    .prop(self.main_axis_alignment, |declarations, value| {
      declarations.set(
        "justify-content",
        StyleValue::Keyword(match value {
          MainAxisAlignment::Start => "flex-start",
          MainAxisAlignment::Center => "center",
          MainAxisAlignment::End => "flex-end",
          MainAxisAlignment::SpaceBetween => "space-between",
          MainAxisAlignment::SpaceAround => "space-around",
          MainAxisAlignment::SpaceEvenly => "space-evenly",
        }),
      )
    })
    .prop(self.cross_axis_alignment, |declarations, value| {
      declarations.set(
        "align-items",
        StyleValue::Keyword(match value {
          CrossAxisAlignment::Start => "flex-start",
          CrossAxisAlignment::End => "flex-end",
          CrossAxisAlignment::Center => "center",
          CrossAxisAlignment::Stretch => "stretch",
          CrossAxisAlignment::Baseline => "baseline",
        }),
      )
    })
    .finish();

    for hook in self.hooks {
      hook(node);
    }

    if self.stack {
      self.children.mount_with(node, |child| {
        with_backend(|backend| backend.set_property(child, "grid-area", "1 / 1"));

        child
      });
    }
    else {
      self.children.mount(node);
    }

    node
  }
}
