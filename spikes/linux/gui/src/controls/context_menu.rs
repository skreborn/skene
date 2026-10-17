//! A context menu: commands offered where the reader asks for them, in a menu hung from the
//! pointer.

use std::rc::Rc;

use skene_foundation::{
  AxisAlignment, Border, Color, Inset, NodeId, State, SystemColors, Value,
  behavior::{Menu, MenuOption},
  layout::Axis,
  seam::{
    access::{AccessFacts, PopupKind},
    control::{Availability, ClipboardOp, LabelPlacement, Part},
    node::{NodeKind, NodeSpec},
    tree::{self, Bubbling},
  },
};

use crate::{
  Active, AsPart, Column, Container, Control, Label, NodeHandle, Overlay, Signal, Text,
  control::control_members,
  view::{Child, Cx, Node, View, child_of},
};

/// What an entry does when chosen.
#[derive(Clone)]
enum Command {
  /// Asks the realization to carry a clipboard operation out on the menu's target.
  Clipboard(ClipboardOp),
  /// Runs the app's own command.
  Run(Rc<dyn Fn()>),
}

/// One command of a [`ContextMenu`]: its label, and what choosing it does.
#[must_use]
#[derive(Clone)]
pub struct MenuEntry {
  label: Rc<str>,
  command: Command,
}

impl MenuEntry {
  /// An entry labelled `label` that runs `run` when chosen; always offered.
  pub fn new(label: impl Into<Rc<str>>, run: impl Fn() + 'static) -> Self {
    Self {
      label: label.into(),
      command: Command::Run(Rc::new(run)),
    }
  }

  /// The standard entry that carries `op` out on the text the menu is opened on, offered as
  /// the platform offers it there ([`skene_foundation::seam::Realization::clipboard_availability`]),
  /// labelled as GTK labels it.
  pub fn clipboard(op: ClipboardOp) -> Self {
    Self {
      label: Rc::from(match op {
        ClipboardOp::Cut => "Cut",
        ClipboardOp::Copy => "Copy",
        ClipboardOp::Paste => "Paste",
        ClipboardOp::Delete => "Delete",
        ClipboardOp::SelectAll => "Select All",
      }),
      command: Command::Clipboard(op),
    }
  }

  /// The entry, labelled `label`.
  pub fn set_label(mut self, label: impl Into<Rc<str>>) -> Self {
    self.label = label.into();

    self
  }

  /// Its label.
  #[must_use]
  pub fn label(&self) -> &str {
    &self.label
  }

  /// Whether it is offered on `target` now.
  fn availability(&self, target: NodeId) -> Availability {
    match self.command {
      Command::Clipboard(op) => tree::clipboard_availability::<Active>(target, op),
      Command::Run(_) => Availability::Enabled,
    }
  }

  /// Carries it out on `target`.
  fn run(&self, target: NodeId) {
    match &self.command {
      Command::Clipboard(op) => tree::clipboard::<Active>(target, *op),
      Command::Run(run) => run(),
    }
  }
}

/// One item of a [`ContextMenu`]'s list, as its item builder makes it: its label's text. Its
/// menu gives it its behavior ([`MenuOption`]) as it builds it; its text is its name.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 at its sides, its corners rounded
/// by 6, its text in [`SystemColors::INK`], filled with [`SystemColors::CONTROL_HOVER`] while
/// focus is on it (where the keys and the pointer move it); a disabled item at half opacity.
#[must_use]
pub struct MenuItem {
  /// The control it composes, its defaults set, over a behavior its menu replaces.
  pub(super) control: Control<MenuOption>,
}

impl MenuItem {
  /// An item labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self {
      control: Control::new(Menu::new().member(0)),
    }
    .set_min_height(30)
    .set_padding(Inset::new(0, 12, 0, 12))
    .set_corner_radius(6)
    .set_item_alignment(AxisAlignment::Center)
    .set_color(SystemColors::INK)
    .set_background(
      Value::from(Color::TRANSPARENT).when(State::Focused, SystemColors::CONTROL_HOVER),
    )
    .set_label_placement(LabelPlacement::Leading)
    .set_label(Label::new(Text::new(label.clone()).set_max_lines(1)).set_name(label))
  }

  control_members!(control.members);

  /// The control this item is, over `behavior`.
  fn into_control(self, behavior: MenuOption) -> Control<MenuOption> {
    self.control.map_behavior(|_| behavior)
  }
}

/// What makes a menu's item from its entry's label.
type ItemBuilder = Rc<dyn Fn(&str) -> MenuItem>;

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
/// Its list: [`ContextMenu::default_list`], its items [`MenuItem`]s.
///
/// ```
/// use skene_gui::prelude::*;
///
/// ContextMenu::new()
///   .add_entry(MenuEntry::new("Refresh", || {}))
///   .set_child(Text::new("Inbox"));
/// ```
#[must_use]
pub struct ContextMenu {
  entries: Vec<MenuEntry>,
  list: Container,
  builder: ItemBuilder,
  target: Option<NodeHandle>,
  child: Option<Child>,
}

impl Default for ContextMenu {
  fn default() -> Self {
    Self::new()
  }
}

impl ContextMenu {
  /// A menu of no entry: it opens nothing.
  pub fn new() -> Self {
    Self {
      entries: Vec::new(),
      list: Self::default_list(),
      builder: Rc::new(|label| MenuItem::new(label)),
      target: None,
      child: None,
    }
  }

  /// The standard menu of a text: cut, copy, paste, delete and select all
  /// ([`MenuEntry::clipboard`]): the editing entries of GTK's entry menu, which adds an emoji
  /// chooser.
  pub fn standard() -> Self {
    [
      ClipboardOp::Cut,
      ClipboardOp::Copy,
      ClipboardOp::Paste,
      ClipboardOp::Delete,
      ClipboardOp::SelectAll,
    ]
    .into_iter()
    .fold(Self::new(), |menu, op| {
      menu.add_entry(MenuEntry::clipboard(op))
    })
  }

  /// The default list: [`SystemColors::FIELD`] inside a border of 1 in
  /// [`SystemColors::BORDER`], its corners rounded by 8, padded 4.
  pub fn default_list() -> Container {
    Container::new()
      .set_background(SystemColors::FIELD)
      .set_border(Border::new(1))
      .set_border_color(SystemColors::BORDER)
      .set_corner_radius(8)
      .set_padding(Inset::all(4))
  }

  /// Appends `entry`.
  pub fn add_entry(mut self, entry: MenuEntry) -> Self {
    self.entries.push(entry);

    self
  }

  /// Its entries, in place of those it has.
  pub fn set_entries(mut self, entries: impl IntoIterator<Item = MenuEntry>) -> Self {
    self.entries = entries.into_iter().collect();

    self
  }

  /// Whether it has no entry, and so opens nothing.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// The box around the open list, in place of [`ContextMenu::default_list`]; the items are
  /// placed in it, so a child it holds is replaced.
  pub fn set_list(mut self, list: Container) -> Self {
    self.list = list;

    self
  }

  /// Makes each item with `builder`, from its entry's label: a styled [`MenuItem`];
  /// `MenuItem::new(label)` where unset.
  pub fn set_item_builder(mut self, builder: impl Fn(&str) -> MenuItem + 'static) -> Self {
    self.builder = Rc::new(builder);

    self
  }

  /// The node the entries act on and that reports the menu, in place of the one its child
  /// builds: the child's own node, or the control it builds a segment around (a control with
  /// a caption beside it).
  pub const fn set_target(mut self, target: &NodeHandle) -> Self {
    self.target = Some(*target);

    self
  }

  /// The node a menu around `built` acts on where no target is set: `built`, or the first
  /// control in the segment it is.
  fn target_in(built: NodeId) -> NodeId {
    let is_segment =
      |id: NodeId| tree::spec::<Active>(id).is_some_and(|spec| spec.kind == NodeKind::Segment);

    if !is_segment(built) {
      return built;
    }

    let mut pending = tree::children::<Active>(built);

    pending.reverse();

    while let Some(id) = pending.pop() {
      if tree::spec::<Active>(id).is_some_and(|spec| spec.is_control()) {
        return id;
      }

      if is_segment(id) {
        pending.extend(tree::children::<Active>(id).into_iter().rev());
      }
    }

    built
  }

  /// The view it is the context menu of, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl View for ContextMenu {
  fn build(self, cx: &mut Cx) -> Node {
    let Self {
      entries,
      list,
      builder,
      target,
      child,
    } = self;
    let Some(child) = child else {
      return Node(cx.create(NodeSpec::new(NodeKind::Segment)));
    };
    let inner = child(cx);
    let segment = cx.child().create(NodeSpec::new(NodeKind::Segment));

    tree::attach::<Active>(segment, inner.0, 0);

    if entries.is_empty() {
      return Node(segment);
    }

    let built = Self::target_in(inner.0);
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
        tree::focus::<Active>(target());
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

        if let Some(entry) = entries.get(index) {
          entry.run(at);
        }
      });
    }

    let items = Column::new().add_children(entries.iter().enumerate().map(|(index, entry)| {
      let member = Menu::with_roster(menu.roster()).member(index);

      builder(entry.label())
        .derive_disabled(move || offered.get().get(index) != Some(&Availability::Enabled))
        .into_control(member)
    }));
    let overlay = Overlay::popup()
      .derive_point(move || point.get())
      .derive_open(move || open.get())
      .handle_dismiss({
        let close = Rc::clone(&close);

        move || close()
      })
      .set_child(
        AsPart::new(Part::List)
          .set_child(list.set_child(Control::new(menu).set_axis(Axis::Vertical).set_child(items))),
      );
    let overlay = overlay.build(&mut cx.child());

    tree::attach::<Active>(segment, overlay.0, 1);

    {
      let entries = Rc::clone(&entries);

      tree::handle_context_menu::<Active, _>(segment, move |event| {
        if open.get_untracked() {
          return Bubbling::Stop;
        }

        let at = target();
        let availability: Vec<Availability> =
          entries.iter().map(|entry| entry.availability(at)).collect();

        if availability
          .iter()
          .all(|availability| *availability == Availability::Absent)
        {
          return Bubbling::Continue;
        }

        let first = availability
          .iter()
          .position(|availability| *availability == Availability::Enabled);

        offered.set(availability);

        #[expect(clippy::cast_possible_truncation, reason = "window pixels")]
        point.set(Some((
          event.window_x.round() as i32,
          event.window_y.round() as i32,
        )));

        open.set(true);

        if let Some(first) = first {
          roster.focus(first);
        }

        Bubbling::Stop
      });
    }

    let reported = target();

    tree::add_access::<Active>(reported, || AccessFacts {
      has_popup: Some(PopupKind::Menu),
      ..AccessFacts::default()
    });

    Node(segment)
  }
}
