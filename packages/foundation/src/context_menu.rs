//! A context menu: commands shown beside the pointer on a secondary press over the
//! view it serves, or beside that view from the keyboard.

use std::rc::Rc;

use skene_reactive::{Effect, Get, Set, Signal, untrack};

use crate::{
  AnyKind,
  AnyView,
  Arrangement,
  Availability,
  ClipboardAction,
  Color,
  Column,
  Condition,
  Container,
  Control,
  DeclaredBoxStyle,
  DecorationBase,
  Dimension,
  FlexBase,
  Handler,
  HasBoxStyle,
  HasChildren,
  Inset,
  ItemAlignment,
  KeyEvent,
  KeyHandler,
  Label,
  Menu,
  MenuOption,
  MotionBase,
  MouseCursor,
  NoChild,
  NodeId,
  Overlay,
  Part,
  Placement,
  Platform,
  Point,
  PointerEvent,
  Prevent,
  Row,
  RunBase,
  Scalar,
  Scene,
  Shortcut,
  Spacing,
  StaticLeaf,
  StyleExtension,
  Text,
  Transition,
  Value,
  View,
  Visibility,
  Wraps,
  properties,
  realize::Source,
};

/// What a [`MenuEntry`] is.
enum ItemKind {
  /// A command of the menu's own, running its handler.
  Command,
  /// A standard clipboard action the platform performs.
  Standard(ClipboardAction),
  /// A rule between groups of items.
  Separator,
}

/// A view drawing a menu item from its content.
type ItemView<P> = Rc<dyn Fn(MenuItemContent) -> AnyView<P>>;

/// What a menu item's view draws from: the item's label and its shortcut.
#[derive(Clone)]
#[must_use]
pub struct MenuItemContent {
  label: Rc<dyn Fn() -> String>,
  shortcut: Option<Shortcut>,
}

impl std::fmt::Debug for MenuItemContent {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("MenuItemContent")
      .finish_non_exhaustive()
  }
}

impl MenuItemContent {
  /// The item's label, as it reads now; tracked where the label is derived.
  #[inline(always)]
  #[must_use]
  pub fn label(&self) -> String {
    (self.label)()
  }

  /// The item's shortcut, if it has one; its `Display` reads as a hint.
  #[inline(always)]
  #[must_use]
  pub const fn shortcut(&self) -> Option<Shortcut> {
    self.shortcut
  }
}

/// One entry of a [`ContextMenu`]: a command with a label, a standard clipboard
/// action, or a separator.
///
/// A command runs [`handle_activate`](Self::handle_activate) when activated. A standard
/// action ([`copy`](Self::copy), [`cut`](Self::cut), [`paste`](Self::paste)) acts on
/// what the platform holds selected or focused, is listed only where the platform
/// offers it, and is disabled while it does not apply.
#[must_use]
pub struct MenuEntry {
  kind: ItemKind,
  label: Source<String>,
  shortcut: Option<Shortcut>,
  disabled: Option<Source<bool>>,
  on_select: Option<Rc<dyn Fn()>>,
}

impl std::fmt::Debug for MenuEntry {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("MenuEntry").finish_non_exhaustive()
  }
}

impl MenuEntry {
  fn with(kind: ItemKind, label: Source<String>) -> Self {
    Self {
      kind,
      label,
      shortcut: None,
      disabled: None,
      on_select: None,
    }
  }

  /// A command labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    Self::with(ItemKind::Command, Source::Fixed(label.into()))
  }

  /// A command whose label is recomputed whenever a signal `label` reads changes.
  pub fn derive(label: impl Fn() -> String + 'static) -> Self {
    Self::with(ItemKind::Command, Source::derive(label))
  }

  /// A rule between groups of items.
  #[inline(always)]
  pub fn separator() -> Self {
    Self::with(ItemKind::Separator, Source::Fixed(String::new()))
  }

  /// The standard `Copy`: writes the selection to the clipboard.
  #[inline(always)]
  pub fn copy() -> Self {
    Self::with(
      ItemKind::Standard(ClipboardAction::Copy),
      Source::Fixed("Copy".to_owned()),
    )
  }

  /// The standard `Cut`: writes the focused entry's selection to the clipboard and
  /// removes it.
  #[inline(always)]
  pub fn cut() -> Self {
    Self::with(
      ItemKind::Standard(ClipboardAction::Cut),
      Source::Fixed("Cut".to_owned()),
    )
  }

  /// The standard `Paste`: inserts the clipboard's text into the focused entry.
  #[inline(always)]
  pub fn paste() -> Self {
    Self::with(
      ItemKind::Standard(ClipboardAction::Paste),
      Source::Fixed("Paste".to_owned()),
    )
  }

  /// Replaces the label, a standard item's included.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Source::Fixed(label.into());

    self
  }

  /// Binds `shortcut` to the item: pressed while focus is inside the view the menu
  /// serves, it runs the item as choosing it does, in place of the platform's
  /// response, and it shows beside the label.
  #[inline(always)]
  pub const fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.shortcut = Some(shortcut);

    self
  }

  /// Sets whether the item is listed but cannot run.
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.disabled = Some(Source::Fixed(disabled));

    self
  }

  /// Derives whether the item is disabled from the signals `disabled` reads.
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Some(Source::derive(disabled));

    self
  }

  /// Runs `action` when the item is activated.
  pub fn handle_activate(mut self, action: impl Fn() + 'static) -> Self {
    self.on_select = Some(Rc::new(action));

    self
  }
}

/// A context menu of [`MenuEntry`]s over the view it serves.
///
/// A secondary press over the view opens it beside the pointer, and `Shift-F10` or the
/// `ContextMenu` key beside the view while focus is inside it; the platform's own menu
/// stays everywhere the view does not reach.
///
/// The menu is a popup [`Overlay`], the [`Part::List`], holding a [`Menu`] of
/// [`MenuOption`] controls, the [`Part::Item`]s: the arrows, `Home`, `End`, typing,
/// `Enter` and `Escape` come from those behaviors, an item runs on release as a
/// button does and closes the menu, and a press outside dismisses it. The deepest
/// menu along the path from the pressed node wins, so a menu on a root serves
/// everything no closer menu does. A menu whose items are all disabled or absent does
/// not open; the platform's menu is prevented either way.
///
/// The list is decorated like a container and every item per property through the
/// `set_item_*` setters. Holds [`NoChild`] until [`set_child`](Self::set_child)
/// supplies the view served; [`serve`](Self::serve) attaches the menu to a node that
/// already exists instead.
#[must_use]
pub struct ContextMenu<P: Platform, C = NoChild> {
  items: Vec<MenuEntry>,
  list: Overlay<P>,
  item_style: Box<DeclaredBoxStyle<P>>,
  item_text_color: Option<Declared<P, Color>>,
  shortcut_color: Option<Declared<P, Color>>,
  separator_color: Option<Declared<P, Color>>,
  separator_thickness: Option<Declared<P, Dimension<P::Scalar>>>,
  shortcut_spacing: Option<Declared<P, Spacing<P::Scalar>>>,
  item_view: Option<ItemView<P>>,
  child: C,
}

impl<P: Platform, C> std::fmt::Debug for ContextMenu<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ContextMenu")
      .finish_non_exhaustive()
  }
}

impl<P: Platform> ContextMenu<P> {
  /// A menu with no items yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      items: Vec::new(),
      list: Overlay::popup().set_placement(Placement::TopStart),
      item_style: Box::default(),
      item_text_color: None,
      shortcut_color: None,
      separator_color: None,
      separator_thickness: None,
      shortcut_spacing: None,
      item_view: None,
      child: NoChild,
    }
  }

  /// Sets the items, in order.
  pub fn set_items(mut self, items: impl IntoIterator<Item = MenuEntry>) -> Self {
    self.items = items.into_iter().collect();

    self
  }

  /// Sets the view the menu serves.
  pub fn set_child(self, child: impl View<P>) -> ContextMenu<P, AnyView<P>> {
    ContextMenu {
      items: self.items,
      list: self.list,
      item_style: self.item_style,
      item_text_color: self.item_text_color,
      shortcut_color: self.shortcut_color,
      separator_color: self.separator_color,
      separator_thickness: self.separator_thickness,
      shortcut_spacing: self.shortcut_spacing,
      item_view: self.item_view,
      child: child.into_any(),
    }
  }

  /// Attaches the menu to `node`, a view that already exists, mounting the list under
  /// it; returns the list's node.
  #[inline(always)]
  pub fn serve(self, node: NodeId) -> NodeId {
    let list = self.mount(node);

    Scene::<P>::with(|scene| scene.attach(node, list, None));

    list
  }
}

impl<P: Platform> Default for ContextMenu<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> ContextMenu<P> {
  /// The declared style of every item, for a facade's medium-specific setters to
  /// write into.
  #[inline(always)]
  pub const fn item_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    &mut self.item_style
  }

  properties! { item_style_mut, P;
    set_item_padding / derive_item_padding (padding) => padding: crate::Inset<P::Scalar>,
      "the padding on each edge of every item";
    set_item_color / derive_item_color (color) => background: Color,
      "the background color of every item";
    set_item_cursor / derive_item_cursor (cursor) => cursor: crate::MouseCursor,
      "the mouse cursor shown over every item";
  }

  /// Sets every item's corner radius, a length in the medium's unit, where the medium
  /// rounds corners; a medium without radii accepts and ignores it.
  pub fn set_item_corner_radius(
    mut self,
    radius: impl Into<Value<crate::Length, Condition<P::Scalar>>>,
  ) -> Self
  where
    P::StyleExt: crate::CornerRadius,
  {
    <P::StyleExt as crate::CornerRadius>::set_corner_radius::<crate::Declared<Condition<P::Scalar>>>(
      &mut self.item_style.ext,
      Some(radius.into()),
    );

    self
  }

  /// Sets the color of every item's label.
  pub fn set_item_text_color(
    mut self,
    color: impl Into<Value<Color, Condition<P::Scalar>>>,
  ) -> Self {
    self.item_text_color = Some(color.into());

    self
  }

  /// Sets the color of every item's shortcut hint; the label's color unless set.
  pub fn set_shortcut_color(
    mut self,
    color: impl Into<Value<Color, Condition<P::Scalar>>>,
  ) -> Self {
    self.shortcut_color = Some(color.into());

    self
  }

  /// Sets the color of every separator.
  pub fn set_separator_color(
    mut self,
    color: impl Into<Value<Color, Condition<P::Scalar>>>,
  ) -> Self {
    self.separator_color = Some(color.into());

    self
  }

  /// Sets the thickness of every separator; one unit of the medium unless told
  /// otherwise.
  pub fn set_separator_thickness(
    mut self,
    thickness: impl Into<Value<Dimension<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    self.separator_thickness = Some(thickness.into());

    self
  }

  /// Sets where the list opens against the pointer; its top-start corner at the
  /// pointer unless told otherwise.
  #[inline(always)]
  pub fn set_placement(mut self, placement: Placement) -> Self {
    self.list = self.list.set_placement(placement);

    self
  }

  /// Sets the view drawing each item from its content; the menu keeps the item's
  /// behavior, its accessible name and when it is listed and enabled. Without one, an
  /// item's label and shortcut sit at its two ends, dressed by the `set_item_*` and
  /// `set_shortcut_*` setters.
  pub fn set_item_view<V: View<P>>(
    mut self,
    view: impl Fn(MenuItemContent) -> V + 'static,
  ) -> Self {
    self.item_view = Some(Rc::new(move |content| view(content).into_any()));

    self
  }

  /// Sets the spacing between an item's caption and its shortcut hint.
  pub fn set_shortcut_spacing(
    mut self,
    spacing: impl Into<Value<Spacing<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    self.shortcut_spacing = Some(spacing.into());

    self
  }

  /// Adds a property transition on the list, interpolating that property whenever its
  /// value changes.
  #[inline(always)]
  pub fn set_transition(
    mut self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    self.list = self.list.set_transition(transition);

    self
  }
}

impl<P: Platform, C> ContextMenu<P, C> {
  /// Builds the list and registers what opens it on `served`; returns the list's node,
  /// attached nowhere yet.
  fn mount(self, served: NodeId) -> NodeId {
    let Self {
      items,
      list,
      item_style,
      item_text_color,
      shortcut_color,
      separator_color,
      separator_thickness,
      shortcut_spacing,
      item_view,
      ..
    } = self;
    let open = Signal::transient(false);
    let position: Signal<Option<Point<f32>>> = Signal::transient(None);
    // Bumped at every request, so a standard item re-asks the platform as the menu opens.
    let generation = Signal::transient(0u32);
    let dress = RowDress {
      item_style,
      item_text_color,
      shortcut_color,
      separator_color,
      separator_thickness,
      shortcut_spacing,
      item_view,
    };
    let mut rows: Vec<AnyView<P>> = Vec::new();
    let mut runs: Vec<Rc<dyn Fn()>> = Vec::new();
    let mut usable: Vec<Predicate> = Vec::new();
    let mut bindings: Vec<(Shortcut, usize)> = Vec::new();

    for item in items {
      if matches!(item.kind, ItemKind::Separator) {
        rows.push(dress.separator());

        continue;
      }

      let value = runs.len().to_string();
      let verdicts = item.verdicts::<P>(generation);
      let Verdicts {
        run,
        shown,
        disabled,
      } = &verdicts;

      if let Some(shortcut) = item.shortcut {
        bindings.push((shortcut, runs.len()));
      }

      rows.push(dress.row(item.label, item.shortcut, value, &verdicts));
      runs.push(Rc::clone(run));
      usable.push({
        let (shown, disabled) = (Rc::clone(shown), Rc::clone(disabled));

        Rc::new(move || shown() && !disabled())
      });
    }

    let menu = Menu::new().handle_choose({
      let runs = runs.clone();

      move |value| {
        open.set(false);

        if let Some(run) = value
          .parse::<usize>()
          .ok()
          .and_then(|index| runs.get(index))
        {
          run();
        }
      }
    });
    let group = menu.roster().clone();
    let list = list
      .derive_open(move || open.get())
      .derive_position(move || position.get())
      .handle_dismiss(move || open.set(false))
      .set_child(Control::<P, _>::new(menu).set_child(Column::<P>::new().set_children(rows)))
      .build();

    let bound = ItemShortcuts {
      bindings,
      runs: runs.clone(),
      usable: usable.clone(),
    }
    .handler(generation);

    Scene::<P>::with(|scene| {
      scene.set_part(list, Part::List);

      if let Some(bound) = bound {
        scene.handle(served, Handler::KeyDown(bound));
      }

      scene.handle(
        served,
        Handler::ContextMenu(Rc::new(move |event: &PointerEvent| {
          generation.set(generation.get_untracked().wrapping_add(1));

          if untrack(|| usable.iter().any(|usable| usable())) {
            position.set(Some(Point::new(event.x, event.y)));
            open.set(true);
          }

          Prevent::All
        })),
      );
    });
    // The first usable item takes focus as the menu opens; an ordinary effect, so the
    // rows and their states have settled by the time it looks.
    Effect::new(move |_: Option<()>| {
      if open.get() {
        untrack(|| group.focus_at::<P>(0));
      }
    });

    list
  }
}

/// The shortcuts a menu's items bind, by the index of the item each runs, with what
/// runs each item and whether it is usable.
struct ItemShortcuts {
  bindings: Vec<(Shortcut, usize)>,
  runs: Vec<Rc<dyn Fn()>>,
  usable: Vec<Predicate>,
}

impl ItemShortcuts {
  /// The key handler running the usable item a key's shortcut names, in place of the
  /// platform's response; `None` where no item binds one. A standard item re-asks the
  /// platform, `generation` moving, before its usability is read.
  fn handler(self, generation: Signal<u32>) -> Option<Rc<KeyHandler>> {
    if self.bindings.is_empty() {
      return None;
    }

    Some(Rc::new(move |event: &KeyEvent| {
      let Some(index) = self
        .bindings
        .iter()
        .find(|(shortcut, _)| event.matches(shortcut))
        .map(|(_, index)| *index)
      else {
        return Prevent::None;
      };

      generation.set(generation.get_untracked().wrapping_add(1));

      if !untrack(|| (self.usable[index])()) {
        return Prevent::None;
      }

      (self.runs[index])();

      Prevent::All
    }))
  }
}

/// A declared value of `T`, its conditions in the platform's scalar.
type Declared<P, T> = Value<T, Condition<<P as Platform>::Scalar>>;

/// A closure answering a yes or no each time it is asked.
type Predicate = Rc<dyn Fn() -> bool>;

/// What an item's row needs from it: what runs it, whether it is listed, and whether
/// its control is disabled.
struct Verdicts {
  run: Rc<dyn Fn()>,
  shown: Predicate,
  disabled: Predicate,
}

impl MenuEntry {
  /// The item's verdicts; a standard item re-asks the platform's clipboard whenever
  /// `generation` moves.
  fn verdicts<P: Platform>(&self, generation: Signal<u32>) -> Verdicts {
    let disabled: Predicate = match &self.disabled {
      None => Rc::new(|| false),
      Some(Source::Fixed(disabled)) => {
        let disabled = *disabled;

        Rc::new(move || disabled)
      }
      Some(Source::Derived(compute)) => Rc::clone(compute),
    };

    if let ItemKind::Standard(action) = self.kind {
      return Verdicts {
        run: Rc::new(move || P::clipboard(action)),
        shown: Rc::new(move || {
          generation.get();

          P::clipboard_availability(action) != Availability::Absent
        }),
        disabled: Rc::new(move || {
          generation.get();

          disabled() || P::clipboard_availability(action) != Availability::Enabled
        }),
      };
    }

    let on_select = self.on_select.clone();

    Verdicts {
      run: Rc::new(move || {
        if let Some(action) = &on_select {
          action();
        }
      }),
      shown: Rc::new(|| true),
      disabled,
    }
  }
}

/// The decoration every row of one menu shares.
struct RowDress<P: Platform> {
  item_style: Box<DeclaredBoxStyle<P>>,
  item_text_color: Option<Declared<P, Color>>,
  shortcut_color: Option<Declared<P, Color>>,
  separator_color: Option<Declared<P, Color>>,
  separator_thickness: Option<Declared<P, Dimension<P::Scalar>>>,
  shortcut_spacing: Option<Declared<P, Spacing<P::Scalar>>>,
  item_view: Option<ItemView<P>>,
}

impl<P: Platform> RowDress<P> {
  /// A rule between groups of items.
  fn separator(&self) -> AnyView<P> {
    let mut rule = Container::<P>::new().set_height(
      self
        .separator_thickness
        .clone()
        .unwrap_or_else(|| Dimension::Length(P::Scalar::from_f32(1.)).into()),
    );

    if let Some(color) = self.separator_color.clone() {
      rule = rule.set_background(color);
    }

    rule.into_any()
  }

  /// One item's row: a [`MenuOption`] carrying `value`, captioned by `label` with
  /// `shortcut` beside it, listed and disabled as `verdicts` say.
  fn row(
    &self,
    label: Source<String>,
    shortcut: Option<Shortcut>,
    value: String,
    verdicts: &Verdicts,
  ) -> AnyView<P> {
    let name = label.clone();
    let row = match &self.item_view {
      Some(view) => view(MenuItemContent {
        label: label.into_read(),
        shortcut,
      }),
      None => self.default_row(label, shortcut),
    };
    let mut control = Control::<P, _>::new(MenuOption::new(value))
      .derive_disabled({
        let disabled = Rc::clone(&verdicts.disabled);

        move || disabled()
      })
      .set_label(match name {
        Source::Fixed(name) => Label::new(row).set_name(name),
        Source::Derived(name) => Label::new(row).derive_name(move || name()),
      });

    *control.box_style_mut() = (*self.item_style).clone();

    Visibility::<P>::new()
      .derive_shown({
        let shown = Rc::clone(&verdicts.shown);

        move || shown()
      })
      .set_child(control.into_any())
      .into_any()
  }

  /// An item's content without a view of the menu's user: the label and the shortcut
  /// at the item's two ends.
  fn default_row(&self, label: Source<String>, shortcut: Option<Shortcut>) -> AnyView<P> {
    let mut caption = match label {
      Source::Fixed(label) => Text::<P>::new(label),
      Source::Derived(compute) => Text::<P>::derive(move || compute()),
    };

    if let Some(color) = self.item_text_color.clone() {
      caption = caption.set_color(color);
    }

    let mut cells = vec![caption.into_any()];

    if let Some(shortcut) = shortcut {
      let mut hint = Text::<P>::new(shortcut.to_string());

      if let Some(color) = self
        .shortcut_color
        .clone()
        .or_else(|| self.item_text_color.clone())
      {
        hint = hint.set_color(color);
      }

      cells.push(hint.into_any());
    }

    let row = Row::<P>::new()
      .set_arrangement(Arrangement::SpaceBetween)
      .set_item_alignment(ItemAlignment::Center)
      .set_spacing(
        self
          .shortcut_spacing
          .clone()
          .unwrap_or_else(|| Spacing::from(P::Scalar::from_f32(2.)).into()),
      )
      .set_children(cells);

    row.into_any()
  }
}

impl<P: Platform> HasBoxStyle<P> for ContextMenu<P> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.list.box_style_mut()
  }
}

impl<P: Platform> DecorationBase<P> for ContextMenu<P> {}

impl<P: Platform, C> AnyKind for ContextMenu<P, C> {}

impl<P: Platform> View<P> for ContextMenu<P, AnyView<P>> {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<P>::create_segment();
    let Self {
      items,
      list,
      item_style,
      item_text_color,
      shortcut_color,
      separator_color,
      separator_thickness,
      shortcut_spacing,
      item_view,
      child,
    } = self;
    let node = child.build();
    let list = ContextMenu::<P> {
      items,
      list,
      item_style,
      item_text_color,
      shortcut_color,
      separator_color,
      separator_thickness,
      shortcut_spacing,
      item_view,
      child: NoChild,
    }
    .mount(node);

    Scene::<P>::with(|scene| {
      scene.insert_in_segment(start, node, None);
      scene.insert_in_segment(start, list, None);
    });

    start
  }
}

/// What a facade's [`ContextMenu`] carries beyond its list's decoration.
///
/// Its constructor, its items, the node it serves, every item's and separator's styling,
/// the item view and the placement.
pub trait ContextMenuBase<P: Platform>: Wraps<Inner = ContextMenu<P>> {
  /// A menu with no items yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(ContextMenu::new())
  }

  /// Sets the items, in order.
  #[must_use]
  fn set_items(self, items: impl IntoIterator<Item = MenuEntry>) -> Self {
    Self::wrap(self.into_inner().set_items(items))
  }

  /// Attaches the menu to `node`, a view that already exists, mounting the list under
  /// it; returns the list's node.
  fn serve(self, node: NodeId) -> NodeId {
    self.into_inner().serve(node)
  }

  /// Sets the padding on each edge of every item.
  #[must_use]
  fn set_item_padding(
    self,
    padding: impl Into<Value<Inset<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    Self::wrap(self.into_inner().set_item_padding(padding))
  }

  /// Derives the padding on each edge of every item from the signals `padding` reads: a value, a
  /// token or an expression.
  #[must_use]
  fn derive_item_padding<R: Into<StaticLeaf<Inset<P::Scalar>>>>(
    self,
    padding: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_item_padding(padding))
  }

  /// Sets the background color of every item.
  #[must_use]
  fn set_item_color(self, color: impl Into<Value<Color, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_item_color(color))
  }

  /// Derives the background color of every item from the signals `color` reads: a value, a token or
  /// an expression.
  #[must_use]
  fn derive_item_color<R: Into<StaticLeaf<Color>>>(self, color: impl Fn() -> R + 'static) -> Self {
    Self::wrap(self.into_inner().derive_item_color(color))
  }

  /// Sets the mouse cursor shown over every item.
  #[must_use]
  fn set_item_cursor(self, cursor: impl Into<Value<MouseCursor, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_item_cursor(cursor))
  }

  /// Derives the mouse cursor shown over every item from the signals `cursor` reads: a value, a
  /// token or an expression.
  #[must_use]
  fn derive_item_cursor<R: Into<StaticLeaf<MouseCursor>>>(
    self,
    cursor: impl Fn() -> R + 'static,
  ) -> Self {
    Self::wrap(self.into_inner().derive_item_cursor(cursor))
  }

  /// Sets the color of every item's label.
  #[must_use]
  fn set_item_text_color(self, color: impl Into<Value<Color, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_item_text_color(color))
  }

  /// Sets the color of every item's shortcut hint; the label's color unless set.
  #[must_use]
  fn set_shortcut_color(self, color: impl Into<Value<Color, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_shortcut_color(color))
  }

  /// Sets the color of every separator.
  #[must_use]
  fn set_separator_color(self, color: impl Into<Value<Color, Condition<P::Scalar>>>) -> Self {
    Self::wrap(self.into_inner().set_separator_color(color))
  }

  /// Sets the thickness of every separator; one unit of the medium unless told
  /// otherwise.
  #[must_use]
  fn set_separator_thickness(
    self,
    thickness: impl Into<Value<Dimension<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    Self::wrap(self.into_inner().set_separator_thickness(thickness))
  }

  /// Sets where the list opens against the pointer; its top-start corner at the
  /// pointer unless told otherwise.
  #[must_use]
  fn set_placement(self, placement: Placement) -> Self {
    Self::wrap(self.into_inner().set_placement(placement))
  }

  /// Sets the view drawing each item from its content; the menu keeps the item's
  /// behavior, its accessible name and when it is listed and enabled. Without
  /// one, an item's label and shortcut sit at its two ends, dressed by the
  /// `set_item_*` and `set_shortcut_*` setters.
  #[must_use]
  fn set_item_view<V: View<P>>(self, view: impl Fn(MenuItemContent) -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_item_view(view))
  }

  /// Sets the spacing between an item's caption and its shortcut hint.
  #[must_use]
  fn set_shortcut_spacing(
    self,
    spacing: impl Into<Value<Spacing<P::Scalar>, Condition<P::Scalar>>>,
  ) -> Self {
    Self::wrap(self.into_inner().set_shortcut_spacing(spacing))
  }

  /// Adds a property transition on the list, interpolating that property whenever
  /// its value changes.
  #[must_use]
  fn set_transition(
    self,
    transition: Transition<<P::StyleExt as StyleExtension>::Property>,
  ) -> Self {
    Self::wrap(self.into_inner().set_transition(transition))
  }
}

/// The corner radius of a facade's [`ContextMenu`] items, where the style rounds corners.
pub trait ContextMenuWithCornerRadius<P: Platform>: ContextMenuBase<P>
where
  P::StyleExt: crate::CornerRadius,
{
  /// Sets every item's corner radius, a length in the medium's unit.
  #[must_use]
  fn set_item_corner_radius(
    self,
    radius: impl Into<Value<crate::Length, Condition<P::Scalar>>>,
  ) -> Self {
    Self::wrap(self.into_inner().set_item_corner_radius(radius))
  }
}

impl<P: Platform, T: ContextMenuBase<P>> ContextMenuWithCornerRadius<P> for T where
  P::StyleExt: crate::CornerRadius
{
}
