//! The shared views: a decorated box, the layouts, text, an image and a theme.
//!
//! Each property has one setter and its `derive_` twin, and conditions ride the value
//! (`set_padding(24.when(Media::MinWidth(900), 48))`). Properties come first, children last.

use std::{hash::Hash, rc::Rc};

use skene_foundation::{
  Alignment, Arrangement, AxisAlignment, Border, Color, Corners, Definitions, Dimension, Effect,
  Flow, FontFamily, FontStyle, FontWeight, Inset, Offset, Ring, Shadows, Signal, Spacing,
  TextAlignment, TextDecoration, TextDecorationStyle, TokenSet, Tracks,
  seam::{
    access::{AccessFacts, Role, Shortcut},
    content::contents,
    node::{LayoutKind, NodeKind, NodeSpec},
    record::{Record, Transition},
    report::{KeyEvent, PointerEvent},
    tree::{self, Bubbling},
  },
};

use crate::{
  Active,
  view::{
    Child, ChildOf, Cx, Node, NodeHandle, View, attach_children, child_of, finish, handle, keyed,
    properties,
  },
};

/// A box's handler of an event: whether the event goes on to the boxes around it.
type Handler<E> = Rc<dyn Fn(&E) -> Bubbling>;

/// The decorated box around at most one child: the one view that takes decoration, sizing
/// and a click.
///
/// To decorate a layout, wrap it: `Container::new().set_background(..).set_child(Column::new())`.
#[derive(Default)]
#[must_use]
pub struct Container {
  record: Record,
  label: Option<Rc<dyn Fn() -> Rc<str>>>,
  click: Option<Rc<dyn Fn()>>,
  shortcut: Option<Shortcut>,
  key: Option<Handler<KeyEvent>>,
  pointer: Option<Handler<PointerEvent>>,
  focus: Option<Rc<dyn Fn(bool)>>,
  handle: Option<NodeHandle>,
  child: Option<Child>,
}

impl Container {
  /// An empty box.
  pub fn new() -> Self {
    Self::default()
  }

  properties! {
    /// Space between the box's edges and its child, in pixels.
    set_padding, derive_padding: PADDING, Inset;
    /// The fill behind the child.
    set_background, derive_background: BACKGROUND, Color;
    /// The corners' radii, in pixels.
    set_corner_radius, derive_corner_radius: CORNER_RADIUS, Corners;
    /// A line around the box, inside its edges.
    set_border, derive_border: BORDER, Border;
    /// The color of the line around the box, its text color where unset.
    set_border_color, derive_border_color: BORDER_COLOR, Color;
    /// The shadows the box casts.
    set_shadow, derive_shadow: SHADOW, Shadows;
    /// The box's opacity, 0 to 1.
    set_opacity, derive_opacity: OPACITY, f32;
    /// A line around the box that takes no room and is never hit, as a focus ring:
    /// `Ring::NONE.when(State::FocusVisible, Ring::new(2))`.
    set_ring, derive_ring: RING, Ring;
    /// The ring's color, the box's text color where unset.
    set_ring_color, derive_ring_color: RING_COLOR, Color;
    /// How far the box and its child are drawn from where they were laid out, taking no room:
    /// in pixels, or in fractions of the box's own size (`Offset::of_size(1., 0.)`).
    set_offset, derive_offset: OFFSET, Offset;
    /// The width.
    set_width, derive_width: WIDTH, Dimension;
    /// The height.
    set_height, derive_height: HEIGHT, Dimension;
    /// The least width.
    set_min_width, derive_min_width: MIN_WIDTH, Dimension;
    /// The least height.
    set_min_height, derive_min_height: MIN_HEIGHT, Dimension;
    /// The greatest width.
    set_max_width, derive_max_width: MAX_WIDTH, Dimension;
    /// The greatest height.
    set_max_height, derive_max_height: MAX_HEIGHT, Dimension;
    /// Where the child sits in the box.
    set_alignment, derive_alignment: ALIGNMENT, Alignment;
    /// Whether what is drawn outside the box's edges (and its rounded corners) is cut off.
    set_clip, derive_clip: CLIP, bool;
    /// The color of the text inside, which every text beneath inherits.
    set_color, derive_color: COLOR, Color;
    /// The font's size inside, in pixels, which every text beneath inherits.
    set_font_size, derive_font_size: FONT_SIZE, f32;
    /// The font's weight inside, which every text beneath inherits.
    set_font_weight, derive_font_weight: FONT_WEIGHT, FontWeight;
    /// Upright or slanted inside, which every text beneath inherits.
    set_font_style, derive_font_style: FONT_STYLE, FontStyle;
    /// The typeface inside, which every text beneath inherits.
    set_font_family, derive_font_family: FONT_FAMILY, FontFamily;
    /// The height of a line inside, as a multiple of the font's size, which every text beneath
    /// inherits.
    set_line_height, derive_line_height: LINE_HEIGHT, f32;
    /// Extra space between letters inside, in pixels, which every text beneath inherits.
    set_letter_spacing, derive_letter_spacing: LETTER_SPACING, f32;
    /// The caret of the text entries inside, which every entry beneath inherits.
    set_caret_color, derive_caret_color: CARET_COLOR, Color;
    /// The fill behind the selection of the text entries inside, which every entry beneath
    /// inherits.
    set_selection_color, derive_selection_color: SELECTION_COLOR, Color;
    /// The placeholder of the empty text entries inside, which every entry beneath inherits.
    set_placeholder_color, derive_placeholder_color: PLACEHOLDER_COLOR, Color;
  }

  /// Animates changes of a property of the box's own look as `transition` says
  /// (`Transition::new(keys::BACKGROUND, 200)`); one of the same property is replaced.
  pub fn add_transition(mut self, transition: Transition) -> Self {
    self.record.add_transition(transition);

    self
  }

  /// Names the box for a reader, as a region.
  pub fn set_label(self, label: impl Into<Rc<str>>) -> Self {
    let label = label.into();

    self.derive_label(move || Rc::clone(&label))
  }

  /// Names the box for a reader, as a region, kept current.
  pub fn derive_label<S: Into<Rc<str>>>(mut self, label: impl Fn() -> S + 'static) -> Self {
    self.label = Some(Rc::new(move || label().into()));

    self
  }

  /// Runs `click` when the box is activated: clicked, or `Enter` or `Space` pressed while it
  /// has focus. A box that acts is an activatable row: a tab stop of its own, a row to a
  /// reader, and it may hold controls, which own their presses.
  pub fn handle_click(mut self, click: impl Fn() + 'static) -> Self {
    self.click = Some(Rc::new(click));

    self
  }

  /// Activates the box, as a click does, when `shortcut` is pressed wherever focus is in its
  /// window, where the box acts ([`Container::handle_click`]); a reader reads it as the box's
  /// key binding.
  pub const fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.shortcut = Some(shortcut);

    self
  }

  /// Runs `key` with each key the controls inside did not consume, while focus is in the box;
  /// `Bubbling::Stop` keeps it from the boxes around it.
  pub fn handle_key<B: Into<Bubbling>>(mut self, key: impl Fn(&KeyEvent) -> B + 'static) -> Self {
    self.key = Some(Rc::new(move |event| key(event).into()));

    self
  }

  /// Runs `pointer` with each press and release in the box, outside the controls inside;
  /// `Bubbling::Stop` keeps it from the boxes around it.
  pub fn handle_pointer<B: Into<Bubbling>>(
    mut self,
    pointer: impl Fn(&PointerEvent) -> B + 'static,
  ) -> Self {
    self.pointer = Some(Rc::new(move |event| pointer(event).into()));

    self
  }

  /// Runs `focus` when focus enters the box (`true`) or leaves it (`false`).
  pub fn handle_focus(mut self, focus: impl Fn(bool) + 'static) -> Self {
    self.focus = Some(Rc::new(focus));

    self
  }

  handle!();

  /// The box's child, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl View for Container {
  fn build(self, cx: &mut Cx) -> Node {
    let mut spec = NodeSpec::new(NodeKind::Layout(LayoutKind::Box));

    if self.label.is_some() {
      spec = spec.with_role(Role::Region);
    }

    if self.click.is_some() {
      spec = spec.acting();
    }

    let id = tree::create::<Active>(spec, &cx.scope);

    finish(id, self.record, cx, self.handle.as_ref());

    if let Some(click) = self.click {
      tree::handle_activate::<Active>(id, move || click());
    }

    if let Some(key) = self.key {
      tree::handle_key::<Active, _>(id, move |event| key(event));
    }

    if let Some(pointer) = self.pointer {
      tree::handle_pointer::<Active, _>(id, move |event| pointer(event));
    }

    if let Some(focus) = self.focus {
      tree::handle_focus::<Active>(id, move |focused| focus(focused));
    }

    if let Some(label) = self.label {
      tree::add_access::<Active>(id, move || AccessFacts {
        name: Some(label()),
        ..AccessFacts::default()
      });
    }

    if let Some(shortcut) = self.shortcut {
      tree::add_access::<Active>(id, move || AccessFacts {
        shortcut: Some(shortcut),
        ..AccessFacts::default()
      });
    }

    attach_children(id, self.child.into_iter().collect(), cx);

    Node(id)
  }
}

/// A layout view: its kind, its record, its children, in the order written.
macro_rules! layout {
  ($(#[doc = $doc:literal])* $name:ident, $kind:expr, { $($properties:tt)* }) => {
    $(#[doc = $doc])*
    #[derive(Default)]
    #[must_use]
    pub struct $name {
      record: Record,
      handle: Option<NodeHandle>,
      children: Vec<Child>,
    }

    impl $name {
      /// An empty layout.
      pub fn new() -> Self {
        Self::default()
      }

      properties! { $($properties)* }

      handle!();

      /// Appends a child, after the properties.
      pub fn add_child(mut self, child: impl ChildOf<Self>) -> Self {
        self.children.push(child_of::<Self>(child));

        self
      }

      /// Appends a child per view `children` yields.
      pub fn add_children<C: ChildOf<Self>>(
        mut self,
        children: impl IntoIterator<Item = C>,
      ) -> Self {
        self
          .children
          .extend(children.into_iter().map(child_of::<Self>));

        self
      }

      /// Appends a keyed list: one child per item, built once per key and kept, wherever it
      /// moves, while its key stays in the list. The children after it follow it wherever its
      /// length puts them.
      pub fn derive_children<T: 'static, K: Eq + Hash + Clone + 'static, C: ChildOf<Self>>(
        mut self,
        items: impl Fn() -> Vec<T> + 'static,
        key: impl Fn(&T) -> K + 'static,
        view: impl Fn(T) -> C + 'static,
      ) -> Self {
        self.children.push(keyed::<T, K, C, Self>(items, key, view));

        self
      }
    }

    impl View for $name {
      fn build(self, cx: &mut Cx) -> Node {
        let id = tree::create::<Active>(NodeSpec::new(NodeKind::Layout($kind)), &cx.scope);

        finish(id, self.record, cx, self.handle.as_ref());
        attach_children(id, self.children, cx);

        Node(id)
      }
    }
  };
}

layout!(
  /// Children side by side.
  Row,
  LayoutKind::Row,
  {
    /// Space between the row's edges and its children, in pixels.
    set_padding, derive_padding: PADDING, Inset;
    /// Space between neighboring children (and between lines, where it wraps), in pixels.
    set_spacing, derive_spacing: SPACING, Spacing;
    /// How the children are distributed along the row.
    set_arrangement, derive_arrangement: ARRANGEMENT, Arrangement;
    /// Where the children sit vertically.
    set_item_alignment, derive_item_alignment: ITEM_ALIGNMENT, AxisAlignment;
    /// Whether the children wrap onto more lines.
    set_wrap, derive_wrap: WRAP, bool;
    /// How the lines are distributed, where the row wraps.
    set_line_arrangement, derive_line_arrangement: LINE_ARRANGEMENT, Arrangement;
  }
);

layout!(
  /// Children one above the other.
  Column,
  LayoutKind::Column,
  {
    /// Space between the column's edges and its children, in pixels.
    set_padding, derive_padding: PADDING, Inset;
    /// Space between neighboring children, in pixels.
    set_spacing, derive_spacing: SPACING, Spacing;
    /// How the children are distributed along the column.
    set_arrangement, derive_arrangement: ARRANGEMENT, Arrangement;
    /// Where the children sit horizontally.
    set_item_alignment, derive_item_alignment: ITEM_ALIGNMENT, AxisAlignment;
  }
);

layout!(
  /// Children in tracks: columns and rows, placed in order or where a `GridCell` says.
  Grid,
  LayoutKind::Grid,
  {
    /// Space between the grid's edges and its children, in pixels.
    set_padding, derive_padding: PADDING, Inset;
    /// Space between tracks, per axis, in pixels.
    set_spacing, derive_spacing: SPACING, Spacing;
    /// The columns.
    set_columns, derive_columns: COLUMNS, Tracks;
    /// The rows.
    set_rows, derive_rows: ROWS, Tracks;
    /// The columns the grid adds as it places children past its own.
    set_implicit_columns, derive_implicit_columns: IMPLICIT_COLUMNS, Tracks;
    /// The rows the grid adds as it places children past its own.
    set_implicit_rows, derive_implicit_rows: IMPLICIT_ROWS, Tracks;
    /// The order the grid places children in.
    set_flow, derive_flow: FLOW, Flow;
    /// Where the children sit in their cells.
    set_item_alignment, derive_item_alignment: ALIGNMENT, Alignment;
  }
);

layout!(
  /// Children over one another in one cell, later ones on top; a `Pinned` child keeps its
  /// distance to the edges it names.
  Stack,
  LayoutKind::Stack,
  {
    /// Space between the stack's edges and its children, in pixels.
    set_padding, derive_padding: PADDING, Inset;
    /// Where the children sit.
    set_item_alignment, derive_item_alignment: ALIGNMENT, Alignment;
  }
);

/// The axes a [`Scroll`] scrolls along.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ScrollAxes {
  /// Up and down: the child is as wide as the scroll and as tall as it likes.
  #[default]
  Vertical,
  /// Side to side: the child is as tall as the scroll and as wide as it likes.
  Horizontal,
  /// Both ways.
  Both,
}

/// A box whose one child scrolls, with the platform's scrolling: its wheel and touchpad
/// motion, its scroll bar.
///
/// It would be as large as its child and can shrink to nothing, so a scroll in a column takes
/// a height, a greatest height or a `Flexible` weight to stay within the window.
#[derive(Default)]
#[must_use]
pub struct Scroll {
  axes: ScrollAxes,
  record: Record,
  handle: Option<NodeHandle>,
  child: Option<Child>,
}

impl Scroll {
  /// A scroll along `axes`, empty.
  pub fn new(axes: ScrollAxes) -> Self {
    Self {
      axes,
      ..Self::default()
    }
  }

  properties! {
    /// The width.
    set_width, derive_width: WIDTH, Dimension;
    /// The height.
    set_height, derive_height: HEIGHT, Dimension;
    /// The least width.
    set_min_width, derive_min_width: MIN_WIDTH, Dimension;
    /// The least height.
    set_min_height, derive_min_height: MIN_HEIGHT, Dimension;
    /// The greatest width.
    set_max_width, derive_max_width: MAX_WIDTH, Dimension;
    /// The greatest height.
    set_max_height, derive_max_height: MAX_HEIGHT, Dimension;
  }

  handle!();

  /// The child that scrolls, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl View for Scroll {
  fn build(self, cx: &mut Cx) -> Node {
    let (horizontal, vertical) = match self.axes {
      ScrollAxes::Vertical => (false, true),
      ScrollAxes::Horizontal => (true, false),
      ScrollAxes::Both => (true, true),
    };
    let id = tree::create::<Active>(
      NodeSpec::new(NodeKind::Layout(LayoutKind::Scroll {
        horizontal,
        vertical,
      })),
      &cx.scope,
    );

    finish(id, self.record, cx, self.handle.as_ref());
    attach_children(id, self.child.into_iter().collect(), cx);

    Node(id)
  }
}

/// A paragraph of text. Its blanks and newlines are kept, and it wraps to its width.
#[must_use]
pub struct Text {
  content: Box<dyn Fn() -> String>,
  record: Record,
  handle: Option<NodeHandle>,
}

impl Text {
  /// The text `text`.
  pub fn new(text: impl Into<String>) -> Self {
    let text = text.into();

    Self::derive(move || text.clone())
  }

  /// The text `text` returns, recomputed whenever a signal it reads changes.
  pub fn derive<S: Into<String>>(text: impl Fn() -> S + 'static) -> Self {
    Self {
      content: Box::new(move || text().into()),
      record: Record::new(),
      handle: None,
    }
  }

  properties! {
    /// The text's color.
    set_color, derive_color: COLOR, Color;
    /// The font's size, in pixels.
    set_font_size, derive_font_size: FONT_SIZE, f32;
    /// The font's weight.
    set_font_weight, derive_font_weight: FONT_WEIGHT, FontWeight;
    /// Upright or slanted.
    set_font_style, derive_font_style: FONT_STYLE, FontStyle;
    /// The typeface.
    set_font_family, derive_font_family: FONT_FAMILY, FontFamily;
    /// The height of a line, as a multiple of the font's size.
    set_line_height, derive_line_height: LINE_HEIGHT, f32;
    /// Extra space between letters, in pixels.
    set_letter_spacing, derive_letter_spacing: LETTER_SPACING, f32;
    /// The lines the text is decorated with.
    set_decoration, derive_decoration: DECORATION, TextDecoration;
    /// How the decoration's lines are drawn.
    set_decoration_style, derive_decoration_style: DECORATION_STYLE, TextDecorationStyle;
    /// The decoration's color, the text's where unset.
    set_decoration_color, derive_decoration_color: DECORATION_COLOR, Color;
    /// Where the lines sit across the paragraph's width.
    set_text_alignment, derive_text_alignment: TEXT_ALIGNMENT, TextAlignment;
    /// The most lines shown, the rest elided.
    set_max_lines, derive_max_lines: MAX_LINES, u32;
  }

  /// Animates changes of a property of the text's look as `transition` says
  /// (`Transition::new(keys::COLOR, 200)`); one of the same property is replaced.
  pub fn add_transition(mut self, transition: Transition) -> Self {
    self.record.add_transition(transition);

    self
  }

  handle!();
}

impl View for Text {
  fn build(self, cx: &mut Cx) -> Node {
    let id = tree::create::<Active>(NodeSpec::new(NodeKind::Text), &cx.scope);
    let content = self.content;

    finish(id, self.record, cx, self.handle.as_ref());
    tree::bind::<Active, _>(id, contents::TEXT, move || content().into());

    Node(id)
  }
}

/// A picture.
#[must_use]
pub struct Image {
  source: Rc<str>,
  description: Option<Rc<dyn Fn() -> Rc<str>>>,
  record: Record,
  handle: Option<NodeHandle>,
}

impl Image {
  /// The picture at `source`, a path the platform resolves among the app's resources.
  pub fn new(source: impl Into<Rc<str>>) -> Self {
    Self {
      source: source.into(),
      description: None,
      record: Record::new(),
      handle: None,
    }
  }

  properties! {
    /// The width.
    set_width, derive_width: WIDTH, Dimension;
    /// The height.
    set_height, derive_height: HEIGHT, Dimension;
  }

  /// Describes the picture for a reader; without one, it is decoration a reader skips.
  pub fn set_description(self, description: impl Into<Rc<str>>) -> Self {
    let description = description.into();

    self.derive_description(move || Rc::clone(&description))
  }

  /// Describes the picture for a reader, kept current; without one, it is decoration a reader
  /// skips.
  pub fn derive_description<S: Into<Rc<str>>>(
    mut self,
    description: impl Fn() -> S + 'static,
  ) -> Self {
    self.description = Some(Rc::new(move || description().into()));

    self
  }

  handle!();
}

impl View for Image {
  fn build(self, cx: &mut Cx) -> Node {
    let (source, description) = (self.source, self.description);
    let spec = match description {
      Some(_) => NodeSpec::new(NodeKind::Image),
      None => NodeSpec::new(NodeKind::Image).with_role(Role::Presentation),
    };
    let id = tree::create::<Active>(spec, &cx.scope);

    finish(id, self.record, cx, self.handle.as_ref());
    tree::bind::<Active, _>(id, contents::SOURCE, move || Rc::clone(&source));

    if let Some(description) = description {
      tree::add_access::<Active>(id, move || AccessFacts {
        name: Some(description()),
        ..AccessFacts::default()
      });
    }

    Node(id)
  }
}

/// Defines a palette's tokens for its child and everything beneath it.
///
/// A token resolves to its definition in the nearest theme above; nesting a theme redefines
/// beneath it. A palette that changes (`Theme::derive`) is rewritten where it is defined, and
/// nothing beneath it is rebuilt.
#[must_use]
pub struct Theme {
  definitions: Box<dyn Fn() -> Definitions>,
  child: Option<Child>,
}

impl Theme {
  /// Defines `palette`'s tokens.
  pub fn new(palette: impl TokenSet) -> Self {
    let definitions = palette.definitions();

    Self {
      definitions: Box::new(move || definitions.clone()),
      child: None,
    }
  }

  /// Defines the tokens of the palette `palette` returns, kept current.
  pub fn derive<S: TokenSet>(palette: impl Fn() -> S + 'static) -> Self {
    Self {
      definitions: Box::new(move || palette().definitions()),
      child: None,
    }
  }

  /// The view the palette is defined for, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl View for Theme {
  fn build(self, cx: &mut Cx) -> Node {
    let definitions = Signal::new(Rc::new(Definitions::new()));
    let compute = self.definitions;

    Effect::new(move || definitions.set(Rc::new(compute())));

    let node = tree::create::<Active>(NodeSpec::new(NodeKind::Scope), &cx.scope);
    let scope = tree::scope::<Active>(&cx.scope, definitions, Some(node));

    if let Some(child) = self.child {
      let child = child(&mut Cx {
        scope,
        placement: cx.placement.clone(),
      });

      tree::attach::<Active>(node, child.0, 0);
    }

    Node(node)
  }
}
