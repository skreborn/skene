//! The shared views: a decorated box, the layouts, a scroll box, text, an image and a theme.
//!
//! Each property has one setter and its `derive_` twin, and conditions ride the value
//! (`set_padding(24.when(Media::MinWidth(900), 48))`). Properties come first, children last.

use std::{hash::Hash, marker::PhantomData, rc::Rc};

use crate::{
  Alignment, Arrangement, AxisAlignment, Color, Definitions, Dimension, Effect, Expr, Flow, Inset,
  Ring, Signal, Spacing, State, SystemColors, TextAlignment, TextDecoration, TextDecorationStyle,
  TokenSet, Tracks, Value, When as _,
  layout::Axis,
  seam::{
    Realization,
    access::{AccessFacts, Role, Shortcut},
    content::contents,
    control::Part,
    node::{Interaction, LayoutKind, NodeKind, NodeSpec},
    record::{Prop, Record, keys},
    report::{ContextMenuEvent, Dropped, KeyEvent, PointerEvent},
    tree::{self, Bubbling},
  },
  view::{
    AnyKind, AsPart, AsPartBase as _, Child, ChildOf, Cx, DecorationBase, HasRecord, MotionBase,
    Node, NodeHandle, RunBase, RunWithEntries, View, Wraps, trait_properties, wraps_itself,
  },
};

/// A box's handler of an event: whether the event goes on to the boxes around it.
type Handler<E> = Rc<dyn Fn(&E) -> Bubbling>;

/// The decorated box around at most one child: the one view that takes decoration, sizing
/// and a click.
///
/// To decorate a layout, wrap it: `Container::new().set_background(..).set_child(Column::new())`.
#[must_use]
pub struct Container<R: Realization> {
  record: Record,
  label: Option<Rc<dyn Fn() -> Rc<str>>>,
  click: Option<Rc<dyn Fn()>>,
  shortcut: Option<Shortcut>,
  key: Option<Handler<KeyEvent>>,
  pointer: Option<Handler<PointerEvent>>,
  context_menu: Option<Handler<ContextMenuEvent>>,
  drop: Option<Handler<Dropped>>,
  focus: Option<Rc<dyn Fn(bool)>>,
  handle: Option<NodeHandle>,
  child: Option<Child<R>>,
}

impl<R: Realization> Default for Container<R> {
  fn default() -> Self {
    Self {
      record: Record::new(),
      label: None,
      click: None,
      shortcut: None,
      key: None,
      pointer: None,
      context_menu: None,
      drop: None,
      focus: None,
      handle: None,
      child: None,
    }
  }
}

impl<R: Realization> Container<R> {
  /// The properties its setters gave it.
  #[must_use]
  pub const fn record(&self) -> &Record {
    &self.record
  }
}

wraps_itself!(Container<R> where R: Realization);

impl<R: Realization> HasRecord for Container<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What a [`Container`] carries beside its decoration, its run and its motion: its alignment,
/// its clip, its name, its handlers and its child.
pub trait ContainerBase<R: Realization>: Wraps<Inner = Container<R>> {
  /// An empty box.
  fn new() -> Self {
    Self::wrap(Container::default())
  }

  trait_properties! {
    /// Where the child sits in the box.
    set_alignment, derive_alignment: ALIGNMENT, Alignment;
    /// Whether what is drawn outside the box's edges (and its rounded corners) is cut off, and
    /// takes no press there.
    set_clip, derive_clip: CLIP, bool;
  }

  /// Names the box for a reader, as a region.
  fn set_label(self, label: impl Into<Rc<str>>) -> Self {
    let label = label.into();

    self.derive_label(move || Rc::clone(&label))
  }

  /// Names the box for a reader, as a region, kept current.
  fn derive_label<S: Into<Rc<str>>>(mut self, label: impl Fn() -> S + 'static) -> Self {
    self.inner_mut().label = Some(Rc::new(move || label().into()));

    self
  }

  /// Runs `click` when the box is activated: clicked, or `Enter` or `Space` pressed while it
  /// has focus. A box that acts is an activatable row: a tab stop of its own, a row to a
  /// reader, and it may hold controls, which own their presses.
  fn handle_click(mut self, click: impl Fn() + 'static) -> Self {
    self.inner_mut().click = Some(Rc::new(click));

    self
  }

  /// Activates the box, as a click does, when `shortcut` is pressed wherever focus is in its
  /// window, where the box acts ([`ContainerBase::handle_click`]); a reader reads it as the
  /// box's key binding.
  fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.inner_mut().shortcut = Some(shortcut);

    self
  }

  /// Runs `key` with each key the controls inside did not consume, while focus is in the box;
  /// `Bubbling::Stop` keeps it from the boxes around it.
  fn handle_key<B: Into<Bubbling>>(mut self, key: impl Fn(&KeyEvent) -> B + 'static) -> Self {
    self.inner_mut().key = Some(Rc::new(move |event| key(event).into()));

    self
  }

  /// Runs `pointer` with each press and release in the box, outside the controls inside;
  /// `Bubbling::Stop` keeps it from the boxes around it.
  fn handle_pointer<B: Into<Bubbling>>(
    mut self,
    pointer: impl Fn(&PointerEvent) -> B + 'static,
  ) -> Self {
    self.inner_mut().pointer = Some(Rc::new(move |event| pointer(event).into()));

    self
  }

  /// Runs `menu` when a context menu is asked for in the box: a secondary press, `Shift-F10`
  /// or the menu key while focus is inside, or a long press on a touchscreen;
  /// `Bubbling::Stop` keeps it from the boxes around it. The platform's own menu is kept away
  /// wherever a box heard it.
  fn handle_context_menu<B: Into<Bubbling>>(
    mut self,
    menu: impl Fn(&ContextMenuEvent) -> B + 'static,
  ) -> Self {
    self.inner_mut().context_menu = Some(Rc::new(move |event| menu(event).into()));

    self
  }

  /// Takes what is dropped in the box, from this app or another; `Bubbling::Stop` keeps it
  /// from the boxes around it. Text and URIs dragged over the box are accepted, the platform
  /// showing that they would drop.
  fn handle_drop<B: Into<Bubbling>>(mut self, drop: impl Fn(&Dropped) -> B + 'static) -> Self {
    self.inner_mut().drop = Some(Rc::new(move |dropped| drop(dropped).into()));

    self
  }

  /// Runs `focus` when focus enters the box (`true`) or leaves it (`false`).
  fn handle_focus(mut self, focus: impl Fn(bool) + 'static) -> Self {
    self.inner_mut().focus = Some(Rc::new(focus));

    self
  }

  /// Writes the box's node into `handle` once built.
  fn set_handle(mut self, handle: &NodeHandle) -> Self {
    self.inner_mut().handle = Some(*handle);

    self
  }

  /// The box's child, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(Child::view(child));

    self
  }
}

impl<R: Realization> ContainerBase<R> for Container<R> {}

impl<R: Realization> DecorationBase for Container<R> {}

impl<R: Realization> RunBase for Container<R> {}

impl<R: Realization> RunWithEntries for Container<R> {}

impl<R: Realization> MotionBase for Container<R> {}

impl<R: Realization> AnyKind for Container<R> {}

impl<R: Realization> View<R> for Container<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let mut spec = NodeSpec::new(NodeKind::Layout(LayoutKind::Box));

    if self.label.is_some() {
      spec = spec.with_role(Role::Region);
    }

    if self.click.is_some() {
      spec = spec.acting();
    }

    let id = cx.create(spec);
    let mut record = self.record;

    if self.click.is_some() && !record.has(Prop::Ring) {
      // A box that acts shows where focus is, as a control does, unless it says otherwise.
      record.set(
        keys::RING,
        Ring::NONE.when(State::FocusVisible, Ring::new(2)),
      );

      if !record.has(Prop::RingColor) {
        record.set(keys::RING_COLOR, SystemColors::ACCENT);
      }
    }

    cx.finish(id, record, self.handle.as_ref());

    if let Some(click) = self.click {
      tree::handle_activate::<R>(id, move || click());
    }

    if let Some(key) = self.key {
      tree::handle_key::<R, _>(id, move |event| key(event));
    }

    if let Some(pointer) = self.pointer {
      tree::handle_pointer::<R, _>(id, move |event| pointer(event));
    }

    if let Some(menu) = self.context_menu {
      tree::handle_context_menu::<R, _>(id, move |event| menu(event));
    }

    if let Some(drop) = self.drop {
      tree::handle_drop::<R, _>(id, move |dropped| drop(dropped));
    }

    if let Some(focus) = self.focus {
      tree::handle_focus::<R>(id, move |focused| focus(focused));
    }

    if let Some(label) = self.label {
      tree::add_access::<R>(id, move || AccessFacts {
        name: Some(label()),
        ..AccessFacts::default()
      });
    }

    if let Some(shortcut) = self.shortcut {
      tree::add_access::<R>(id, move || AccessFacts {
        shortcut: Some(shortcut),
        ..AccessFacts::default()
      });
    }

    cx.attach_children(id, self.child.into_iter().collect());

    Node::new(id)
  }
}

/// A layout view: its kind, its record, its children, in the order written, and its `Base`
/// trait of the properties it honors.
macro_rules! layout {
  (
    $(#[doc = $doc:literal])* $name:ident, $base:ident, $kind:expr,
    { $($properties:tt)* }
  ) => {
    $(#[doc = $doc])*
    #[must_use]
    pub struct $name<R: Realization> {
      record: Record,
      handle: Option<NodeHandle>,
      children: Vec<Child<R>>,
    }

    wraps_itself!($name<R> where R: Realization);

    impl<R: Realization> HasRecord for $name<R> {
      #[inline(always)]
      fn record_mut(&mut self) -> &mut Record {
        &mut self.record
      }
    }

    #[doc = concat!("What a [`", stringify!($name), "`] carries: its layout properties, its handle and its children.")]
    pub trait $base<R: Realization>: Wraps<Inner = $name<R>> {
      /// An empty layout.
      fn new() -> Self {
        Self::wrap($name {
          record: Record::new(),
          handle: None,
          children: Vec::new(),
        })
      }

      trait_properties! { $($properties)* }

      /// Writes the layout's node into `handle` once built.
      fn set_handle(mut self, handle: &NodeHandle) -> Self {
        self.inner_mut().handle = Some(*handle);

        self
      }

      /// Appends a child, after the properties.
      fn add_child(mut self, child: impl ChildOf<$name<R>, R>) -> Self {
        self.inner_mut().children.push(Child::of::<$name<R>>(child));

        self
      }

      /// Appends a child per view `children` yields.
      fn add_children<C: ChildOf<$name<R>, R>>(
        mut self,
        children: impl IntoIterator<Item = C>,
      ) -> Self {
        self
          .inner_mut()
          .children
          .extend(children.into_iter().map(Child::of::<$name<R>>));

        self
      }

      /// Appends a keyed list: one child per item, built once per key and kept, wherever it
      /// moves, while its key stays in the list. The children after it follow it wherever its
      /// length puts them.
      fn derive_children<T: 'static, K: Eq + Hash + Clone + 'static, C: ChildOf<$name<R>, R>>(
        mut self,
        items: impl Fn() -> Vec<T> + 'static,
        key: impl Fn(&T) -> K + 'static,
        view: impl Fn(T) -> C + 'static,
      ) -> Self {
        self
          .inner_mut()
          .children
          .push(Child::keyed::<T, K, C, $name<R>>(items, key, view));

        self
      }
    }

    impl<R: Realization> $base<R> for $name<R> {}

    impl<R: Realization> AnyKind for $name<R> {}

    impl<R: Realization> View<R> for $name<R> {
      fn build(self, cx: &mut Cx<R>) -> Node {
        let id = cx.create(NodeSpec::new(NodeKind::Layout($kind)));

        cx.finish(id, self.record, self.handle.as_ref());
        cx.attach_children(id, self.children);

        Node::new(id)
      }
    }
  };
}

layout!(
  /// Children side by side.
  Row,
  RowBase,
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
  ColumnBase,
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
  GridBase,
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
  StackBase,
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

/// What makes a scroll bar's part along an axis.
type BarBuilder<R> = Rc<dyn Fn(Axis) -> Container<R>>;

/// A box whose one child scrolls, with the platform's scrolling: its wheel and touchpad
/// motion, its scroll bars.
///
/// It would be as large as its child and can shrink to nothing, so a scroll in a column takes
/// a height, a greatest height or a `Flexible` weight to stay within the window.
///
/// Where the platform draws its bars ([`Realization::DRAWS_SCROLL_BARS`]), a bar along each
/// axis it scrolls shows while the content overflows it, over the content and taking no room:
/// a track ([`Scroll::default_track`]) along the box's trailing or bottom edge, holding a thumb
/// ([`Scroll::default_thumb`]) whose length is the track's times the share of the content in
/// view, its least size along the axis a floor, and whose place along the track is the
/// scroll's. The scroll box is the bar's control: a part's `Hovered` holds while the pointer is
/// over the box, its `Pressed` while a press inside it is held.
#[must_use]
pub struct Scroll<R: Realization> {
  axes: ScrollAxes,
  record: Record,
  handle: Option<NodeHandle>,
  track: BarBuilder<R>,
  thumb: BarBuilder<R>,
  child: Option<Child<R>>,
}

impl<R: Realization> Scroll<R> {
  /// The default track along `axis`: 10 pixels thick, padded 3 at its sides and 2 at its
  /// ends, so its thumb runs 3 pixels in from the box's edge.
  pub fn default_track(axis: Axis) -> Container<R> {
    match axis {
      Axis::Vertical => Container::new()
        .set_width(10)
        .set_padding(Inset::new(2, 3, 2, 3)),
      Axis::Horizontal => Container::new()
        .set_height(10)
        .set_padding(Inset::new(3, 2, 3, 2)),
    }
  }

  /// The default thumb along `axis`: 4 pixels thick and at least 20 long, its corners rounded
  /// by 2, in the ink at an alpha of 70 of 255, 110 while the pointer is over the scroll box.
  pub fn default_thumb(axis: Axis) -> Container<R> {
    let thumb = Container::new().set_corner_radius(2).set_background(
      Value::from(Expr::<Color>::ink().with_alpha(70. / 255.))
        .when(State::Hovered, Expr::<Color>::ink().with_alpha(110. / 255.)),
    );

    match axis {
      Axis::Vertical => thumb.set_width(4).set_min_height(20),
      Axis::Horizontal => thumb.set_height(4).set_min_width(20),
    }
  }
}

wraps_itself!(Scroll<R> where R: Realization);

impl<R: Realization> HasRecord for Scroll<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What a [`Scroll`] carries: its axes, its sizing, its handle, its bars' parts and its child.
pub trait ScrollBase<R: Realization>: Wraps<Inner = Scroll<R>> {
  /// A scroll along `axes`, empty.
  fn new(axes: ScrollAxes) -> Self {
    Self::wrap(Scroll {
      axes,
      record: Record::new(),
      handle: None,
      track: Rc::new(Scroll::<R>::default_track),
      thumb: Rc::new(Scroll::<R>::default_thumb),
      child: None,
    })
  }

  trait_properties! {
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

  /// Writes the scroll's node into `handle` once built.
  fn set_handle(mut self, handle: &NodeHandle) -> Self {
    self.inner_mut().handle = Some(*handle);

    self
  }

  /// Makes the bar's track along each axis with `builder`; [`Scroll::default_track`] where
  /// unset. The thumb is placed in it, so a child it holds is replaced.
  fn set_track_builder<C: Wraps<Inner = Container<R>>>(
    mut self,
    builder: impl Fn(Axis) -> C + 'static,
  ) -> Self {
    self.inner_mut().track = Rc::new(move |axis| builder(axis).into_inner());

    self
  }

  /// Makes the bar's thumb along each axis with `builder`; [`Scroll::default_thumb`] where
  /// unset. Its length and its place along the track are the scroll's, its least size along
  /// the axis a floor under its length.
  fn set_thumb_builder<C: Wraps<Inner = Container<R>>>(
    mut self,
    builder: impl Fn(Axis) -> C + 'static,
  ) -> Self {
    self.inner_mut().thumb = Rc::new(move |axis| builder(axis).into_inner());

    self
  }

  /// The child that scrolls, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(Child::view(child));

    self
  }
}

impl<R: Realization> ScrollBase<R> for Scroll<R> {}

impl<R: Realization> AnyKind for Scroll<R> {}

impl<R: Realization> View<R> for Scroll<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let (horizontal, vertical) = match self.axes {
      ScrollAxes::Vertical => (false, true),
      ScrollAxes::Horizontal => (true, false),
      ScrollAxes::Both => (true, true),
    };
    let id = cx.create(NodeSpec::new(NodeKind::Layout(LayoutKind::Scroll {
      horizontal,
      vertical,
    })));
    let children: Vec<Child<R>> = self.child.into_iter().collect();
    let content = children.len();

    cx.finish(id, self.record, self.handle.as_ref());
    cx.attach_children(id, children);

    if !R::DRAWS_SCROLL_BARS {
      return Node::new(id);
    }

    let axes = [(vertical, Axis::Vertical), (horizontal, Axis::Horizontal)];

    for (index, axis) in axes
      .into_iter()
      .filter_map(|(scrolls, axis)| scrolls.then_some(axis))
      .enumerate()
    {
      let mut bar = cx.child();

      bar.set_control(id);
      bar.set_part(Part::Track);
      bar.set_interaction(Interaction::ScrollBar { axis });

      let track = (self.track)(axis)
        .set_child(AsPart::<R>::new(Part::Thumb).set_child((self.thumb)(axis)))
        .build(&mut bar);

      tree::attach::<R>(id, track.id(), content + index);
    }

    Node::new(id)
  }
}

/// What returns the texts a text lays out as wide as.
type Reserved = Rc<dyn Fn() -> Rc<[Rc<str>]>>;

/// A paragraph of text. Its blanks and newlines are kept, and it wraps to its width.
#[must_use]
pub struct Text<R: Realization> {
  content: Box<dyn Fn() -> String>,
  reserved: Option<Reserved>,
  record: Record,
  handle: Option<NodeHandle>,
  realization: PhantomData<fn() -> R>,
}

wraps_itself!(Text<R> where R: Realization);

impl<R: Realization> HasRecord for Text<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What a [`Text`] carries beside its run and its motion: its content, the texts it is as
/// wide as, its decoration, its alignment, its line limit and its handle.
pub trait TextBase<R: Realization>: Wraps<Inner = Text<R>> {
  /// The text `text`.
  fn new(text: impl Into<String>) -> Self {
    let text = text.into();

    Self::derive(move || text.clone())
  }

  /// The text `text` returns, recomputed whenever a signal it reads changes.
  fn derive<S: Into<String>>(text: impl Fn() -> S + 'static) -> Self {
    Self::wrap(Text {
      content: Box::new(move || text().into()),
      reserved: None,
      record: Record::new(),
      handle: None,
      realization: PhantomData,
    })
  }

  /// Lays the text out as wide as the widest of `texts` and itself, in its own style: a face
  /// showing one of several values keeps one width whichever it shows.
  fn set_reserved_texts<S: Into<Rc<str>>>(self, texts: impl IntoIterator<Item = S>) -> Self {
    let texts: Rc<[Rc<str>]> = texts.into_iter().map(Into::into).collect();

    self.derive_reserved_texts(move || texts.to_vec())
  }

  /// Lays the text out as wide as the widest of what `texts` returns and itself, kept current.
  fn derive_reserved_texts<S: Into<Rc<str>>, I: IntoIterator<Item = S>>(
    mut self,
    texts: impl Fn() -> I + 'static,
  ) -> Self {
    self.inner_mut().reserved = Some(Rc::new(move || {
      texts().into_iter().map(Into::into).collect()
    }));

    self
  }

  trait_properties! {
    /// The lines the text is decorated with.
    set_decoration, derive_decoration: DECORATION, TextDecoration;
    /// How the decoration's lines are drawn.
    set_decoration_style, derive_decoration_style: DECORATION_STYLE, TextDecorationStyle;
    /// The decoration's color, the text's where unset.
    set_decoration_color, derive_decoration_color: DECORATION_COLOR, crate::Color;
    /// Where the lines sit across the paragraph's width.
    set_text_alignment, derive_text_alignment: TEXT_ALIGNMENT, TextAlignment;
    /// The most lines shown, the rest elided.
    set_max_lines, derive_max_lines: MAX_LINES, u32;
  }

  /// Writes the text's node into `handle` once built.
  fn set_handle(mut self, handle: &NodeHandle) -> Self {
    self.inner_mut().handle = Some(*handle);

    self
  }
}

impl<R: Realization> TextBase<R> for Text<R> {}

impl<R: Realization> RunBase for Text<R> {}

impl<R: Realization> MotionBase for Text<R> {}

impl<R: Realization> AnyKind for Text<R> {}

impl<R: Realization> View<R> for Text<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let id = cx.create(NodeSpec::new(NodeKind::Text));
    let content = self.content;

    cx.finish(id, self.record, self.handle.as_ref());
    tree::bind::<R, _>(id, contents::TEXT, move || content().into());

    if let Some(reserved) = self.reserved {
      tree::bind::<R, _>(id, contents::RESERVE, move || reserved());
    }

    Node::new(id)
  }
}

/// A picture.
#[must_use]
pub struct Image<R: Realization> {
  source: Rc<str>,
  description: Option<Rc<dyn Fn() -> Rc<str>>>,
  record: Record,
  handle: Option<NodeHandle>,
  realization: PhantomData<fn() -> R>,
}

wraps_itself!(Image<R> where R: Realization);

impl<R: Realization> HasRecord for Image<R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.record
  }
}

/// What an [`Image`] carries: its source, its size, its description and its handle.
pub trait ImageBase<R: Realization>: Wraps<Inner = Image<R>> {
  /// The picture at `source`, a path the platform resolves among the app's resources.
  fn new(source: impl Into<Rc<str>>) -> Self {
    Self::wrap(Image {
      source: source.into(),
      description: None,
      record: Record::new(),
      handle: None,
      realization: PhantomData,
    })
  }

  trait_properties! {
    /// The width.
    set_width, derive_width: WIDTH, Dimension;
    /// The height.
    set_height, derive_height: HEIGHT, Dimension;
  }

  /// Describes the picture for a reader; without one, it is decoration a reader skips.
  fn set_description(self, description: impl Into<Rc<str>>) -> Self {
    let description = description.into();

    self.derive_description(move || Rc::clone(&description))
  }

  /// Describes the picture for a reader, kept current; without one, it is decoration a reader
  /// skips.
  fn derive_description<S: Into<Rc<str>>>(mut self, description: impl Fn() -> S + 'static) -> Self {
    self.inner_mut().description = Some(Rc::new(move || description().into()));

    self
  }

  /// Writes the image's node into `handle` once built.
  fn set_handle(mut self, handle: &NodeHandle) -> Self {
    self.inner_mut().handle = Some(*handle);

    self
  }
}

impl<R: Realization> ImageBase<R> for Image<R> {}

impl<R: Realization> AnyKind for Image<R> {}

impl<R: Realization> View<R> for Image<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let (source, description) = (self.source, self.description);
    let spec = match description {
      Some(_) => NodeSpec::new(NodeKind::Image),
      None => NodeSpec::new(NodeKind::Image).with_role(Role::Presentation),
    };
    let id = cx.create(spec);

    cx.finish(id, self.record, self.handle.as_ref());
    tree::bind::<R, _>(id, contents::SOURCE, move || Rc::clone(&source));

    if let Some(description) = description {
      tree::add_access::<R>(id, move || AccessFacts {
        name: Some(description()),
        ..AccessFacts::default()
      });
    }

    Node::new(id)
  }
}

/// Defines a palette's tokens for its child and everything beneath it.
///
/// A token resolves to its definition in the nearest theme above; nesting a theme redefines
/// beneath it. A palette that changes (`Theme::derive`) is rewritten where it is defined, and
/// nothing beneath it is rebuilt.
#[must_use]
pub struct Theme<R: Realization> {
  definitions: Box<dyn Fn() -> Definitions>,
  child: Option<Child<R>>,
}

wraps_itself!(Theme<R> where R: Realization);

/// What a [`Theme`] carries: its palette and its child.
pub trait ThemeBase<R: Realization>: Wraps<Inner = Theme<R>> {
  /// Defines `palette`'s tokens.
  fn new(palette: impl TokenSet) -> Self {
    let definitions = palette.definitions();

    Self::wrap(Theme {
      definitions: Box::new(move || definitions.clone()),
      child: None,
    })
  }

  /// Defines the tokens of the palette `palette` returns, kept current.
  fn derive<S: TokenSet>(palette: impl Fn() -> S + 'static) -> Self {
    Self::wrap(Theme {
      definitions: Box::new(move || palette().definitions()),
      child: None,
    })
  }

  /// The view the palette is defined for, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(Child::view(child));

    self
  }
}

impl<R: Realization> ThemeBase<R> for Theme<R> {}

impl<R: Realization> AnyKind for Theme<R> {}

impl<R: Realization> View<R> for Theme<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let definitions = Signal::new(Rc::new(Definitions::new()));
    let compute = self.definitions;

    Effect::new(move || definitions.set(Rc::new(compute())));

    let node = cx.create(NodeSpec::new(NodeKind::Scope));
    let scope = tree::scope::<R>(cx.scope(), definitions, Some(node));

    if let Some(child) = self.child {
      let mut inner = cx.inherit();

      inner.set_scope(scope);

      let child = child.build(&mut inner);

      tree::attach::<R>(node, child.id(), 0);
    }

    Node::new(node)
  }
}
