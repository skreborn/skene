//! A context menu: commands shown beside the pointer on a secondary press over the
//! view it serves, or beside that view from the keyboard.

use std::rc::Rc;

use skene_core::{
  AnyKind,
  AnyView,
  Arrangement,
  Availability,
  ClipboardAction,
  Color,
  Condition,
  DeclaredBoxStyle,
  Dimension,
  HasChildren,
  IntoView,
  ItemAlignment,
  NoChild,
  NodeId,
  Part,
  Placement,
  Platform,
  Point,
  PointerEvent,
  Prevent,
  Scalar,
  Scene,
  Show,
  Spacing,
  StyleExtension,
  Transition,
  Value,
  View,
};
use skene_reactive::{Effect, Get, Set, Signal, untrack};

use crate::{
  Column,
  Container,
  Control,
  Label,
  Menu,
  MenuOption,
  Overlay,
  Row,
  Text,
  decoration,
  properties,
  realize::Source,
};

/// What a [`MenuItem`] is.
enum ItemKind {
  /// A command of the menu's own, running its handler.
  Command,
  /// A standard clipboard action the platform performs.
  Standard(ClipboardAction),
  /// A rule between groups of items.
  Separator,
}

/// One entry of a [`ContextMenu`]: a command with a label, a standard clipboard
/// action, or a separator.
///
/// A command runs [`handle_select`](Self::handle_select) when chosen. A standard
/// action ([`copy`](Self::copy), [`cut`](Self::cut), [`paste`](Self::paste)) acts on
/// what the platform holds selected or focused, is listed only where the platform
/// offers it, and is disabled while it does not apply.
#[must_use]
pub struct MenuItem {
  kind: ItemKind,
  label: Source<String>,
  shortcut: Option<String>,
  disabled: Option<Source<bool>>,
  on_select: Option<Rc<dyn Fn()>>,
}

impl std::fmt::Debug for MenuItem {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("MenuItem").finish_non_exhaustive()
  }
}

impl MenuItem {
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
  #[inline(always)]
  pub fn new(label: impl Into<String>) -> Self {
    Self::with(ItemKind::Command, Source::Fixed(label.into()))
  }

  /// A command whose label is recomputed whenever a signal `label` reads changes.
  #[inline(always)]
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
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Source::Fixed(label.into());

    self
  }

  /// Sets the shortcut hint shown beside the label; a hint only, binding no key.
  #[inline(always)]
  pub fn set_shortcut(mut self, shortcut: impl Into<String>) -> Self {
    self.shortcut = Some(shortcut.into());

    self
  }

  /// Sets whether the item is listed but cannot run.
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.disabled = Some(Source::Fixed(disabled));

    self
  }

  /// Derives whether the item is disabled from the signals `disabled` reads.
  #[inline(always)]
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Some(Source::derive(disabled));

    self
  }

  /// Runs `action` when the item is chosen.
  #[inline(always)]
  pub fn handle_select(mut self, action: impl Fn() + 'static) -> Self {
    self.on_select = Some(Rc::new(action));

    self
  }
}

/// A context menu of [`MenuItem`]s over the view it serves.
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
#[derive(Debug)]
#[must_use]
pub struct ContextMenu<P: Platform, C = NoChild> {
  items: Vec<MenuItem>,
  list: Overlay<P>,
  item_style: DeclaredBoxStyle<P>,
  item_text_color: Option<Declared<P, Color>>,
  shortcut_color: Option<Declared<P, Color>>,
  separator_color: Option<Declared<P, Color>>,
  separator_thickness: Option<Declared<P, Dimension<P::Scalar>>>,
  shortcut_spacing: Option<Declared<P, Spacing<P::Scalar>>>,
  child: C,
}

impl<P: Platform> ContextMenu<P> {
  /// A menu with no items yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      items: Vec::new(),
      list: Overlay::popup().set_placement(Placement::TopStart),
      item_style: DeclaredBoxStyle::<P>::default(),
      item_text_color: None,
      shortcut_color: None,
      separator_color: None,
      separator_thickness: None,
      shortcut_spacing: None,
      child: NoChild,
    }
  }

  /// Sets the items, in order.
  #[inline(always)]
  pub fn set_items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
    self.items = items.into_iter().collect();

    self
  }

  /// Sets the view the menu serves.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<P>) -> ContextMenu<P, AnyView<P>> {
    ContextMenu {
      items: self.items,
      list: self.list,
      item_style: self.item_style,
      item_text_color: self.item_text_color,
      shortcut_color: self.shortcut_color,
      separator_color: self.separator_color,
      separator_thickness: self.separator_thickness,
      shortcut_spacing: self.shortcut_spacing,
      child: child.into_view(),
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

impl<P: Platform, C> ContextMenu<P, C> {
  /// The declared style of the list, for a facade's medium-specific setters to write
  /// into.
  #[inline(always)]
  pub const fn list_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.list.style_mut()
  }

  decoration! { list_style_mut }

  /// The declared style of every item, for a facade's medium-specific setters to
  /// write into.
  #[inline(always)]
  pub const fn item_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    &mut self.item_style
  }

  properties! { item_style_mut;
    set_item_padding / derive_item_padding (padding) => padding: skene_core::Inset<P::Scalar>,
      "the padding on each edge of every item";
    set_item_color / derive_item_color (color) => color: Color,
      "the background color of every item";
    set_item_cursor / derive_item_cursor (cursor) => cursor: skene_core::MouseCursor,
      "the mouse cursor shown over every item";
  }

  /// Sets every item's corner radius, in the medium's unit, where the medium rounds
  /// corners; a medium without radii accepts and ignores it.
  #[inline(always)]
  pub fn set_item_corner_radius(
    mut self,
    radius: impl Into<Value<f32, Condition<P::Scalar>>>,
  ) -> Self
  where
    P::StyleExt: crate::CornerRadius,
  {
    <P::StyleExt as crate::CornerRadius>::set_corner_radius::<
      skene_core::Declared<Condition<P::Scalar>>,
    >(&mut self.item_style.ext, Some(radius.into()));

    self
  }

  /// Sets the color of every item's label.
  #[inline(always)]
  pub fn set_item_text_color(
    mut self,
    color: impl Into<Value<Color, Condition<P::Scalar>>>,
  ) -> Self {
    self.item_text_color = Some(color.into());

    self
  }

  /// Sets the color of every item's shortcut hint; the label's color unless set.
  #[inline(always)]
  pub fn set_shortcut_color(
    mut self,
    color: impl Into<Value<Color, Condition<P::Scalar>>>,
  ) -> Self {
    self.shortcut_color = Some(color.into());

    self
  }

  /// Sets the color of every separator.
  #[inline(always)]
  pub fn set_separator_color(
    mut self,
    color: impl Into<Value<Color, Condition<P::Scalar>>>,
  ) -> Self {
    self.separator_color = Some(color.into());

    self
  }

  /// Sets the thickness of every separator; one unit of the medium unless told
  /// otherwise.
  #[inline(always)]
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

  /// Sets the spacing between an item's caption and its shortcut hint.
  #[inline(always)]
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
      ..
    } = self;
    let open = Signal::opaque(false);
    let position: Signal<Option<Point<f32>>> = Signal::opaque(None);
    // Bumped at every request, so a standard item re-asks the platform as the menu opens.
    let generation = Signal::opaque(0u32);
    let dress = RowDress {
      item_style,
      item_text_color,
      shortcut_color,
      separator_color,
      separator_thickness,
      shortcut_spacing,
    };
    let mut rows: Vec<AnyView<P>> = Vec::new();
    let mut runs: Vec<Rc<dyn Fn()>> = Vec::new();
    let mut usable: Vec<Predicate> = Vec::new();

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

      rows.push(dress.row(item.label, item.shortcut, value, &verdicts));
      runs.push(Rc::clone(run));
      usable.push({
        let (shown, disabled) = (Rc::clone(shown), Rc::clone(disabled));

        Rc::new(move || shown() && !disabled())
      });
    }

    let menu = Menu::new().handle_activate({
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

    Scene::<P>::with(|scene| {
      scene.set_part(list, Part::List);
      scene.handle_context_menu(
        served,
        Rc::new(move |event: &PointerEvent| {
          generation.set(generation.get_untracked().wrapping_add(1));

          if untrack(|| usable.iter().any(|usable| usable())) {
            position.set(Some(Point::new(event.x, event.y)));
            open.set(true);
          }

          Prevent::All
        }),
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

impl MenuItem {
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
        run: Rc::new(move || P::default().clipboard(action)),
        shown: Rc::new(move || {
          generation.get();

          P::default().clipboard_availability(action) != Availability::Absent
        }),
        disabled: Rc::new(move || {
          generation.get();

          disabled() || P::default().clipboard_availability(action) != Availability::Enabled
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
  item_style: DeclaredBoxStyle<P>,
  item_text_color: Option<Declared<P, Color>>,
  shortcut_color: Option<Declared<P, Color>>,
  separator_color: Option<Declared<P, Color>>,
  separator_thickness: Option<Declared<P, Dimension<P::Scalar>>>,
  shortcut_spacing: Option<Declared<P, Spacing<P::Scalar>>>,
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
      rule = rule.set_color(color);
    }

    rule.into_view()
  }

  /// One item's row: a [`MenuOption`] carrying `value`, captioned by `label` with
  /// `shortcut` beside it, listed and disabled as `verdicts` say.
  fn row(
    &self,
    label: Source<String>,
    shortcut: Option<String>,
    value: String,
    verdicts: &Verdicts,
  ) -> AnyView<P> {
    let name = label.clone();
    let mut caption = match label {
      Source::Fixed(label) => Text::<P>::new(label),
      Source::Derived(compute) => Text::<P>::derive(move || compute()),
    };

    if let Some(color) = self.item_text_color.clone() {
      caption = caption.set_color(color);
    }

    let mut cells = vec![caption.into_view()];

    if let Some(shortcut) = shortcut {
      let mut hint = Text::<P>::new(shortcut);

      if let Some(color) = self
        .shortcut_color
        .clone()
        .or_else(|| self.item_text_color.clone())
      {
        hint = hint.set_color(color);
      }

      cells.push(hint.into_view());
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
    let mut control = Control::<P, _>::new(MenuOption::new(value))
      .derive_disabled({
        let disabled = Rc::clone(&verdicts.disabled);

        move || disabled()
      })
      .set_label(match name {
        Source::Fixed(name) => Label::new(row).set_name(name),
        Source::Derived(name) => Label::new(row).derive_name(move || name()),
      });

    *control.style_mut() = self.item_style.clone();

    Show::<P>::new()
      .derive_shown({
        let shown = Rc::clone(&verdicts.shown);

        move || shown()
      })
      .set_child(control.into_view())
      .into_view()
  }
}

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
