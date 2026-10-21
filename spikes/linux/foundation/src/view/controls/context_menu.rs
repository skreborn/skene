//! A context menu: commands offered where the reader asks for them, in a menu hung from the
//! pointer.

use std::rc::Rc;

use crate::{
  AxisAlignment, Border, Color, Inset, NodeId, Signal, State, SystemColors, Value,
  behavior::{Menu, MenuOption},
  layout::Axis,
  seam::{
    Realization,
    access::{AccessFacts, PopupKind, Shortcut},
    control::{Availability, ClipboardOp, LabelPlacement, Part},
    node::{NodeKind, NodeSpec},
    tree::{self, Bubbling},
  },
  untrack,
  view::{
    AnyKind, AsPart, AsPartBase, Child, Column, ColumnBase, Container, ContainerBase, ControlBase,
    ControlMembersBase, Cx, DecorationBase, Label, Node, NodeHandle, Overlay, OverlayBase, RunBase,
    Text, TextBase, View, Wraps, controls::control_surfaces, wraps_itself,
  },
};

/// One command of a [`ContextMenu`]: its label, its shortcut, whether it is offered and what
/// choosing it runs. It is data, so a platform that shows its own menus can show it.
///
/// ```
/// use skene_foundation::view::MenuEntry;
///
/// let refresh = MenuEntry::new("Refresh").handle_activate(|| {});
/// let copy = MenuEntry::copy();
///
/// assert_eq!(refresh.label().as_deref(), Some("Refresh"));
/// assert_eq!(copy.label(), None, "labelled as the platform labels it, by its menu");
/// ```
#[must_use]
#[derive(Clone)]
pub struct MenuEntry {
  /// Its label, as it reads now; the platform's for a standard entry given none.
  label: Option<Rc<dyn Fn() -> Rc<str>>>,
  /// The standard operation it carries out on the menu's target, where it is a standard one.
  standard: Option<ClipboardOp>,
  /// What choosing it runs, after its standard operation.
  activate: Option<Rc<dyn Fn()>>,
  /// Whether it is disabled, as it reads now.
  disabled: Option<Rc<dyn Fn() -> bool>>,
  /// The shortcut it reports.
  shortcut: Option<Shortcut>,
}

impl MenuEntry {
  /// A command labelled `label`, always offered; [`MenuEntry::handle_activate`] says what it
  /// runs.
  pub fn new(label: impl Into<Rc<str>>) -> Self {
    let label = label.into();

    Self {
      label: Some(Rc::new(move || Rc::clone(&label))),
      standard: None,
      activate: None,
      disabled: None,
      shortcut: None,
    }
  }

  /// The standard entry carrying `op` out on the text the menu is opened on, offered as the
  /// platform offers it there ([`crate::seam::Realization::clipboard_availability`]), labelled
  /// as the platform labels it ([`crate::seam::Realization::operation_label`]) unless given a
  /// label.
  pub const fn standard(op: ClipboardOp) -> Self {
    Self {
      label: None,
      standard: Some(op),
      activate: None,
      disabled: None,
      shortcut: None,
    }
  }

  /// The standard entry that cuts the selection to the clipboard.
  pub const fn cut() -> Self {
    Self::standard(ClipboardOp::Cut)
  }

  /// The standard entry that copies the selection to the clipboard.
  pub const fn copy() -> Self {
    Self::standard(ClipboardOp::Copy)
  }

  /// The standard entry that pastes the clipboard over the selection.
  pub const fn paste() -> Self {
    Self::standard(ClipboardOp::Paste)
  }

  /// The standard entry that deletes the selection.
  pub const fn delete() -> Self {
    Self::standard(ClipboardOp::Delete)
  }

  /// The standard entry that selects the whole text.
  pub const fn select_all() -> Self {
    Self::standard(ClipboardOp::SelectAll)
  }

  /// Runs `activate` when the entry is chosen; a standard entry's runs after its operation.
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.activate = Some(Rc::new(activate));

    self
  }

  /// Its label.
  pub fn set_label(self, label: impl Into<Rc<str>>) -> Self {
    let label = label.into();

    self.derive_label(move || Rc::clone(&label))
  }

  /// Its label, kept current.
  pub fn derive_label<S: Into<Rc<str>>>(mut self, label: impl Fn() -> S + 'static) -> Self {
    self.label = Some(Rc::new(move || label().into()));

    self
  }

  /// Whether it is disabled: shown, but not chosen.
  pub fn set_disabled(self, disabled: bool) -> Self {
    self.derive_disabled(move || disabled)
  }

  /// Whether it is disabled, kept current.
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Some(Rc::new(disabled));

    self
  }

  /// The shortcut that runs its command elsewhere, which its item reports, and which chooses
  /// it while its menu is open.
  pub const fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.shortcut = Some(shortcut);

    self
  }

  /// Its label as it reads now, tracking what it reads: `None` for a standard entry given
  /// none, which its menu labels as the platform does.
  #[must_use]
  pub fn label(&self) -> Option<Rc<str>> {
    self.label.as_ref().map(|label| label())
  }

  /// Its label on `R`'s platform as it reads now: its own, else the platform's for its
  /// operation.
  fn label_on<R: Realization>(&self) -> Rc<dyn Fn() -> Rc<str>> {
    match (&self.label, self.standard) {
      (Some(label), _) => Rc::clone(label),
      (None, Some(op)) => Rc::new(move || Rc::from(R::operation_label(op))),
      (None, None) => Rc::new(|| Rc::from("")),
    }
  }

  /// Its label as a caption that follows it: a line of text naming its item.
  pub fn caption<R: Realization>(&self) -> Label<R> {
    let (text, name) = (self.label_on::<R>(), self.label_on::<R>());

    Label::new(Text::<R>::derive(move || text().to_string()).set_max_lines(1))
      .derive_name(move || name())
  }

  /// Its shortcut, where it has one.
  #[must_use]
  pub const fn shortcut(&self) -> Option<Shortcut> {
    self.shortcut
  }

  /// Whether the platform offers it on `target` now: a standard entry as the platform
  /// offers its operation there, any other always.
  fn offered<R: Realization>(&self, target: NodeId) -> Availability {
    self.standard.map_or(Availability::Enabled, |op| {
      tree::clipboard_availability::<R>(target, op)
    })
  }

  /// Whether its own state disables it now, tracking what it reads.
  fn is_disabled(&self) -> bool {
    self.disabled.as_ref().is_some_and(|disabled| disabled())
  }

  /// Carries it out on `target`.
  fn run<R: Realization>(&self, target: NodeId) {
    if let Some(op) = self.standard {
      tree::clipboard::<R>(target, op);
    }

    if let Some(activate) = &self.activate {
      activate();
    }
  }
}

/// One item of a [`ContextMenu`]'s list, as its item builder makes it: its label. Its menu
/// gives it its behavior ([`MenuOption`]) and its entry's shortcut as it builds it.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 at its sides, its corners rounded
/// by 6, its text in [`SystemColors::INK`], filled with [`SystemColors::CONTROL_HOVER`] while
/// focus is on it (where the keys and the pointer move it).
#[must_use]
pub struct MenuItem<R: Realization> {
  /// The control it composes, its defaults set, over a behavior its menu replaces.
  control: crate::view::Control<MenuOption, R>,
}

wraps_itself!(MenuItem<R> where R: Realization);

control_surfaces!([R: Realization] MenuItem<R>, R => control);

impl<R: Realization> MenuItem<R> {
  /// The control this item is, over `behavior`.
  fn into_control(self, behavior: MenuOption) -> crate::view::Control<MenuOption, R> {
    self.control.map_behavior(|_| behavior)
  }
}

/// What a [`MenuItem`] carries: every control's members.
pub trait MenuItemBase<R: Realization>: Wraps<Inner = MenuItem<R>> {
  /// An item labelled `label`: a text names it, a view by the texts it shows or through
  /// [`Label::set_name`].
  fn new(label: impl Into<Label<R>>) -> Self {
    let control = crate::view::Control::new(Menu::new().member(0))
      .set_min_height(30)
      .set_padding(Inset::new(0, 12, 0, 12))
      .set_corner_radius(6)
      .set_item_alignment(AxisAlignment::Center)
      .set_color(SystemColors::INK)
      .set_background(
        Value::from(Color::TRANSPARENT).when(State::Focused, SystemColors::CONTROL_HOVER),
      )
      .set_label_placement(LabelPlacement::Leading)
      .set_label(label);

    Self::wrap(MenuItem { control })
  }
}

impl<R: Realization> MenuItemBase<R> for MenuItem<R> {}

/// What makes a menu's item from its entry.
type ItemBuilder<R> = Rc<dyn Fn(&MenuEntry) -> MenuItem<R>>;

/// A context menu around one view: a secondary press, `Shift-F10` or the menu key on the view
/// opens its entries in a menu hung from the pointer (from the focus's middle by keyboard),
/// the deepest menu along the path taking it, and keeps the platform's own from showing. A
/// menu whose every entry is absent there opens nothing and lets the request climb.
///
/// Open, focus is on its first operable item: the arrows move it, wrapping, `Enter`, `Space`
/// and a click run the item and close the menu, `Escape`, `Tab` and a press outside it close
/// it; focus then returns to the view, whose selection the menu kept. The view reports that
/// it opens a menu.
///
/// Its list: [`ContextMenu::default_list`], its items [`MenuItem`]s. Its entries and the view
/// it wraps are its content, so they end its chain.
#[must_use]
pub struct ContextMenu<R: Realization> {
  /// Its entries.
  entries: Vec<MenuEntry>,
  /// The box around the open list.
  list: Container<R>,
  /// What makes each item.
  builder: ItemBuilder<R>,
  /// The node the entries act on, where it is not the child's.
  target: Option<NodeHandle>,
  /// The view it is the menu of.
  child: Option<Child<R>>,
}

wraps_itself!(ContextMenu<R> where R: Realization);

impl<R: Realization> ContextMenu<R> {
  /// The default list: [`SystemColors::FIELD`] inside a border of 1 in
  /// [`SystemColors::BORDER`], its corners rounded by 8, padded 4.
  pub fn default_list() -> Container<R> {
    Container::new()
      .set_background(SystemColors::FIELD)
      .set_border(Border::new(1))
      .set_border_color(SystemColors::BORDER)
      .set_corner_radius(8)
      .set_padding(Inset::all(4))
  }

  /// The node a menu around `built` acts on where no target is set: `built`, or the first
  /// control in the segment it is.
  fn target_in(built: NodeId) -> NodeId {
    let is_segment =
      |id: NodeId| tree::spec::<R>(id).is_some_and(|spec| spec.kind == NodeKind::Segment);

    if !is_segment(built) {
      return built;
    }

    let mut pending = tree::children::<R>(built);

    pending.reverse();

    while let Some(id) = pending.pop() {
      if tree::spec::<R>(id).is_some_and(|spec| spec.is_control()) {
        return id;
      }

      if is_segment(id) {
        pending.extend(tree::children::<R>(id).into_iter().rev());
      }
    }

    built
  }
}

/// What a [`ContextMenu`] carries: its entries, its list, what makes each item, the node it
/// acts on and the view it wraps.
pub trait ContextMenuBase<R: Realization>: Wraps<Inner = ContextMenu<R>> {
  /// A menu of no entry: it opens nothing.
  fn new() -> Self {
    Self::wrap(ContextMenu {
      entries: Vec::new(),
      list: ContextMenu::<R>::default_list(),
      builder: Rc::new(|entry| MenuItem::new(entry.caption())),
      target: None,
      child: None,
    })
  }

  /// The standard menu of a text, as the platform offers it: its operations in its order
  /// ([`crate::seam::Realization::text_menu`]; cut, copy, paste, delete and select all on
  /// Linux, GTK's entry menu's editing entries), each labelled as the platform labels it.
  fn standard() -> Self {
    Self::new().set_entries(R::text_menu().iter().map(|op| MenuEntry::standard(*op)))
  }

  /// Whether it has no entry, and so opens nothing.
  #[must_use]
  fn is_empty(&self) -> bool {
    self.inner().entries.is_empty()
  }

  /// The box around the open list, in place of [`ContextMenu::default_list`]; the items are
  /// placed in it, so a child it holds is replaced.
  fn set_list(mut self, list: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().list = list.into_inner();

    self
  }

  /// Makes each item with `builder`, from its entry: a styled [`MenuItem`];
  /// `MenuItem::new(entry.caption())` where unset.
  fn set_item_builder<I: Wraps<Inner = MenuItem<R>>>(
    mut self,
    builder: impl Fn(&MenuEntry) -> I + 'static,
  ) -> Self {
    self.inner_mut().builder = Rc::new(move |entry| builder(entry).into_inner());

    self
  }

  /// The node the entries act on and that reports the menu, in place of the one its child
  /// builds: the child's own node, or the control it builds a segment around (a control with
  /// a caption beside it).
  fn set_target(mut self, target: &NodeHandle) -> Self {
    self.inner_mut().target = Some(*target);

    self
  }

  /// Appends `entry`.
  fn add_entry(mut self, entry: MenuEntry) -> Self {
    self.inner_mut().entries.push(entry);

    self
  }

  /// Its entries, in place of those it has.
  fn set_entries(mut self, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
    self.inner_mut().entries = entries.into_iter().collect();

    self
  }

  /// The view it is the context menu of, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(Child::view(child));

    self
  }
}

impl<R: Realization> ContextMenuBase<R> for ContextMenu<R> {}

impl<R: Realization> AnyKind for ContextMenu<R> {}

impl<R: Realization> View<R> for ContextMenu<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      entries,
      list,
      builder,
      target,
      child,
    } = self;
    let Some(child) = child else {
      return Node::new(cx.create(NodeSpec::new(NodeKind::Segment)));
    };
    let inner = child.build(cx);
    let segment = cx.child().create(NodeSpec::new(NodeKind::Segment));

    tree::attach::<R>(segment, inner.id(), 0);

    if entries.is_empty() {
      return Node::new(segment);
    }

    let built = Self::target_in(inner.id());
    let target = move || target.and_then(|target| target.node()).unwrap_or(built);
    let entries = Rc::new(entries);
    let open = Signal::new(false);
    let point = Signal::new(None::<(i32, i32)>);
    let offered = Signal::new(vec![Availability::Absent; entries.len()]);
    let menu = Menu::new();
    let roster = menu.roster();
    let close = Rc::new(move || {
      if open.get_untracked() {
        open.set(false);
        tree::focus::<R>(target());
      }
    });

    {
      let close = Rc::clone(&close);

      roster.set_dismiss(move || close());
    }

    {
      let (close, entries) = (Rc::clone(&close), Rc::clone(&entries));

      roster.set_commit(move |index| {
        let at = target();

        close();

        if let Some(entry) = entries.get(*index) {
          entry.run::<R>(at);
        }
      });
    }

    let items =
      Column::<R>::new().add_children(entries.iter().enumerate().map(|(index, entry)| {
        let member = Menu::with_roster(menu.roster()).member(index);
        let disabled = entry.clone();
        let item = builder(entry).derive_disabled(move || {
          offered.get().get(index) != Some(&Availability::Enabled) || disabled.is_disabled()
        });
        let item = match entry.shortcut() {
          Some(shortcut) => item.set_shortcut(shortcut),
          None => item,
        };

        item.into_control(member)
      }));
    let overlay = Overlay::<R>::popup()
      .derive_point(move || point.get())
      .derive_open(move || open.get())
      .handle_dismiss({
        let close = Rc::clone(&close);

        move || close()
      })
      .set_child(
        AsPart::<R>::new(Part::List).set_child(
          list.set_child(
            crate::view::Control::<Menu, R>::new(menu)
              .set_axis(Axis::Vertical)
              .set_child(items),
          ),
        ),
      );
    let overlay = overlay.build(&mut cx.child());

    tree::attach::<R>(segment, overlay.id(), 1);

    {
      let entries = Rc::clone(&entries);

      tree::handle_context_menu::<R, _>(segment, move |event| {
        if open.get_untracked() {
          return Bubbling::Stop;
        }

        let at = target();
        let availability: Vec<Availability> =
          entries.iter().map(|entry| entry.offered::<R>(at)).collect();

        if availability
          .iter()
          .all(|availability| *availability == Availability::Absent)
        {
          return Bubbling::Continue;
        }

        let first = availability
          .iter()
          .zip(entries.iter())
          .position(|(availability, entry)| {
            *availability == Availability::Enabled && !untrack(|| entry.is_disabled())
          });

        offered.set(availability);

        #[expect(clippy::cast_possible_truncation, reason = "window pixels")]
        point.set(Some((
          event.window_x.round() as i32,
          event.window_y.round() as i32,
        )));

        open.set(true);

        if let Some(first) = first {
          roster.focus(&first);
        }

        Bubbling::Stop
      });
    }

    let reported = target();

    tree::add_access::<R>(reported, || AccessFacts {
      has_popup: Some(PopupKind::Menu),
      ..AccessFacts::default()
    });

    Node::new(segment)
  }
}
