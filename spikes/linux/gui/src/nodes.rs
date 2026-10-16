//! The control nodes the shared controls build: each a node of its control kind, its values
//! bound or controlled through the seam. On Linux Skene draws every one of them.

#![expect(
  dead_code,
  reason = "each node takes every setter its kind may; the shared controls forward some"
)]

use std::rc::Rc;

use skene_foundation::{
  Alignment, NodeId,
  seam::{
    access::{Role, Shortcut},
    content::{Choice, Range, contents},
    node::{ControlKind, LayoutKind, NodeKind, NodeSpec},
    record::{Record, keys},
    tree,
  },
};

use crate::{
  Active, Signal,
  access::{Access, access},
  editing::{Editing, editing, enable, enabled},
  view::{Cx, Node, NodeHandle, View, finish, handle, properties},
};

/// Creates a control of `kind` in `cx`, its record `record` with the placement folded in, and
/// writes it into `handle`.
fn control(kind: ControlKind, record: Record, cx: &Cx, handle: Option<&NodeHandle>) -> NodeId {
  let id = tree::create::<Active>(NodeSpec::new(NodeKind::Control(kind)), &cx.scope);

  finish(id, record, cx, handle);

  id
}

/// A switch node.
#[must_use]
pub(crate) struct Switch {
  record: Record,
  on: Rc<dyn Fn() -> bool>,
  change: Option<Rc<dyn Fn(bool)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Default for Switch {
  fn default() -> Self {
    Self::new()
  }
}

impl Switch {
  /// An inactive switch, centered in the room its parent gives it.
  pub fn new() -> Self {
    let mut record = Record::new();

    record.set(keys::SELF_ALIGNMENT, Alignment::CENTER);

    Self {
      record,
      on: Rc::new(|| false),
      change: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// Whether the switch is on.
  pub fn set_on(self, on: bool) -> Self {
    self.derive_on(move || on)
  }

  /// Whether the switch is on, kept current.
  pub fn derive_on(mut self, on: impl Fn() -> bool + 'static) -> Self {
    self.on = Rc::new(on);

    self
  }

  /// Runs `change` with the state the reader flipped the switch to.
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  /// Shows `on` and writes each flip into it.
  pub fn bind_on(self, on: Signal<bool>) -> Self {
    self
      .derive_on(move || on.get())
      .handle_change(move |value| on.set(value))
  }

  properties! {
    /// Where the switch sits in the room its parent gives it (in place of its own `halign` and
    /// `valign`), overriding the parent's item alignment; centered where unset.
    set_alignment, derive_alignment: SELF_ALIGNMENT, Alignment;
  }

  enabled!();
  handle!();
  access!();
}

impl View for Switch {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(ControlKind::Switch, self.record, cx, self.handle.as_ref());
    let (on, change) = (self.on, self.change);

    tree::control::<Active, _>(
      id,
      contents::ON,
      move || on(),
      change.map(|change| Box::new(move |on| change(on)) as Box<dyn Fn(bool)>),
    );

    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A button node.
#[must_use]
pub(crate) struct Button {
  caption: Rc<dyn Fn() -> String>,
  prominent: Option<Rc<dyn Fn() -> bool>>,
  activate: Option<Rc<dyn Fn()>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Button {
  /// A button showing `caption`.
  pub fn new(caption: impl Into<String>) -> Self {
    let caption = caption.into();

    Self::derive(move || caption.clone())
  }

  /// A button showing what `caption` returns, kept current.
  pub fn derive<S: Into<String>>(caption: impl Fn() -> S + 'static) -> Self {
    Self {
      caption: Rc::new(move || caption().into()),
      prominent: None,
      activate: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// Shows the button as the window's primary action.
  pub fn set_prominent(self, prominent: bool) -> Self {
    self.derive_prominent(move || prominent)
  }

  /// Activates the button when `shortcut` is pressed wherever focus is in its window.
  pub const fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.access.shortcut = Some(shortcut);

    self
  }

  /// Shows the button as the window's primary action, kept current.
  pub fn derive_prominent(mut self, prominent: impl Fn() -> bool + 'static) -> Self {
    self.prominent = Some(Rc::new(prominent));

    self
  }

  /// Runs `activate` when the button is activated: clicked, or pressed from the
  /// keyboard.
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.activate = Some(Rc::new(activate));

    self
  }

  enabled!();
  handle!();
  access!();
}

impl View for Button {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(ControlKind::Button, Record::new(), cx, self.handle.as_ref());
    let caption = self.caption;

    tree::bind::<Active, _>(id, contents::CAPTION, move || caption().into());

    if let Some(activate) = self.activate {
      tree::handle_activate::<Active>(id, move || activate());
    }

    if let Some(prominent) = self.prominent {
      tree::bind::<Active, _>(id, contents::PROMINENT, move || prominent());
    }

    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A one-line text entry node: its text and selection controlled, each edit reported as the
/// whole text it proposes.
#[must_use]
pub(crate) struct Entry {
  editing: Editing,
  placeholder: Option<Rc<dyn Fn() -> Rc<str>>>,
  secure: Option<Rc<dyn Fn() -> bool>>,
  keyboard: Option<Rc<dyn Fn() -> &'static str>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Default for Entry {
  fn default() -> Self {
    Self::new()
  }
}

impl Entry {
  /// An empty entry.
  pub fn new() -> Self {
    Self {
      editing: Editing::default(),
      placeholder: None,
      secure: None,
      keyboard: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  editing!(
    text: "",
    change: "",
    selection: "",
    read_only: ""
  );

  /// The text shown while the entry is empty.
  pub fn set_placeholder(self, placeholder: impl Into<Rc<str>>) -> Self {
    let placeholder = placeholder.into();

    self.derive_placeholder(move || Rc::clone(&placeholder))
  }

  /// The text shown while the entry is empty, kept current.
  pub fn derive_placeholder<S: Into<Rc<str>>>(
    mut self,
    placeholder: impl Fn() -> S + 'static,
  ) -> Self {
    self.placeholder = Some(Rc::new(move || placeholder().into()));

    self
  }

  /// Whether what is typed is hidden.
  pub fn set_secure(self, secure: bool) -> Self {
    self.derive_secure(move || secure)
  }

  /// Whether what is typed is hidden, kept current.
  pub fn derive_secure(mut self, secure: impl Fn() -> bool + 'static) -> Self {
    self.secure = Some(Rc::new(secure));

    self
  }

  /// The keyboard the entry asks the input for, by name, kept current.
  pub fn derive_keyboard(mut self, keyboard: impl Fn() -> &'static str + 'static) -> Self {
    self.keyboard = Some(Rc::new(keyboard));

    self
  }

  /// Runs `activate` when the reader presses `Enter` in the entry.
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.editing.activate = Some(Rc::new(activate));

    self
  }

  enabled!();
  handle!();
  access!();
}

impl View for Entry {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(
      ControlKind::TextField,
      Record::new(),
      cx,
      self.handle.as_ref(),
    );

    if let Some(placeholder) = self.placeholder {
      tree::bind::<Active, _>(id, contents::PLACEHOLDER, move || placeholder());
    }

    if let Some(secure) = self.secure {
      tree::bind::<Active, _>(id, contents::SECURE, move || secure());
    }

    if let Some(keyboard) = self.keyboard {
      tree::bind::<Active, _>(id, contents::KEYBOARD, move || Rc::from(keyboard()));
    }

    self.editing.apply(id);
    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A multi-line text entry node, wrapping its lines at word boundaries, controlled as the
/// one-line entry is.
#[must_use]
pub(crate) struct TextView {
  editing: Editing,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Default for TextView {
  fn default() -> Self {
    Self::new()
  }
}

impl TextView {
  /// An empty text view.
  pub fn new() -> Self {
    Self {
      editing: Editing::default(),
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  editing!(text: "", change: "", selection: "", read_only: "");
  enabled!();
  handle!();
  access!();
}

impl View for TextView {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(
      ControlKind::TextEditor,
      Record::new(),
      cx,
      self.handle.as_ref(),
    );

    self.editing.apply(id);
    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A slider node: a value dragged along a range.
#[must_use]
pub(crate) struct Scale {
  value: Rc<dyn Fn() -> f64>,
  range: Rc<dyn Fn() -> (f64, f64)>,
  step: Rc<dyn Fn() -> f64>,
  change: Option<Rc<dyn Fn(f64)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Default for Scale {
  fn default() -> Self {
    Self::new()
  }
}

impl Scale {
  /// A scale at 0 of 0 to 1, stepping by a hundredth.
  pub fn new() -> Self {
    Self {
      value: Rc::new(|| 0.),
      range: Rc::new(|| (0., 1.)),
      step: Rc::new(|| 0.01),
      change: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// The scale's value (its adjustment's `value`).
  pub fn set_value(self, value: f64) -> Self {
    self.derive_value(move || value)
  }

  /// The scale's value (its adjustment's `value`), kept current.
  pub fn derive_value(mut self, value: impl Fn() -> f64 + 'static) -> Self {
    self.value = Rc::new(value);

    self
  }

  /// The least and greatest values (its adjustment's `lower` and `upper`); 0 to 1 where
  /// unset.
  pub fn set_range(self, minimum: f64, maximum: f64) -> Self {
    self.derive_range(move || (minimum, maximum))
  }

  /// The least and greatest values (its adjustment's `lower` and `upper`), kept current.
  pub fn derive_range(mut self, range: impl Fn() -> (f64, f64) + 'static) -> Self {
    self.range = Rc::new(range);

    self
  }

  /// The step the value moves by from the keyboard (its adjustment's `step-increment`); a
  /// hundredth where unset.
  pub fn set_step(self, step: f64) -> Self {
    self.derive_step(move || step)
  }

  /// The step the value moves by from the keyboard (its adjustment's `step-increment`), kept
  /// current.
  pub fn derive_step(mut self, step: impl Fn() -> f64 + 'static) -> Self {
    self.step = Rc::new(step);

    self
  }

  /// Runs `change` with the value the reader moved the scale to, clamped to
  /// the range.
  pub fn handle_change(mut self, change: impl Fn(f64) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  /// Shows `value` and writes each move into it.
  pub fn bind_value(self, value: Signal<f64>) -> Self {
    self
      .derive_value(move || value.get())
      .handle_change(move |moved| value.set(moved))
  }

  enabled!();
  handle!();
  access!();
}

impl View for Scale {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(ControlKind::Slider, Record::new(), cx, self.handle.as_ref());
    let (range, step) = (self.range, self.step);
    let (value, change) = (self.value, self.change);

    tree::control::<Active, _>(
      id,
      contents::RANGE,
      move || {
        let (minimum, maximum) = range();

        Range {
          value: value(),
          minimum,
          maximum,
          step: step(),
        }
      },
      change.map(|change| Box::new(move |range: Range| change(range.value)) as Box<dyn Fn(Range)>),
    );
    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A picker node: one choice of a list, shown closed.
#[must_use]
pub(crate) struct DropDown {
  options: Rc<dyn Fn() -> Rc<[Rc<str>]>>,
  chosen: Rc<dyn Fn() -> Option<u32>>,
  choose: Option<Rc<dyn Fn(Option<u32>)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Default for DropDown {
  fn default() -> Self {
    Self::new()
  }
}

impl DropDown {
  /// A dropdown with no options.
  pub fn new() -> Self {
    Self {
      options: Rc::new(|| Rc::from([])),
      chosen: Rc::new(|| None),
      choose: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// The options' texts.
  pub fn set_options<S: Into<Rc<str>>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Rc<[Rc<str>]> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.to_vec())
  }

  /// The options' texts, kept current.
  pub fn derive_options<S: Into<Rc<str>>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.options = Rc::new(move || options().into_iter().map(Into::into).collect());

    self
  }

  /// The chosen option's index, or none.
  pub fn set_chosen(self, chosen: Option<u32>) -> Self {
    self.derive_chosen(move || chosen)
  }

  /// The chosen option's index, or none, kept current.
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<u32> + 'static) -> Self {
    self.chosen = Rc::new(chosen);

    self
  }

  /// Runs `choose` with the option the reader chose (`notify::selected`).
  pub fn handle_choose(mut self, choose: impl Fn(Option<u32>) + 'static) -> Self {
    self.choose = Some(Rc::new(choose));

    self
  }

  /// Shows `chosen` and writes each choice into it.
  pub fn bind_chosen(self, chosen: Signal<Option<u32>>) -> Self {
    self
      .derive_chosen(move || chosen.get())
      .handle_choose(move |choice| chosen.set(choice))
  }

  enabled!();
  handle!();
  access!();
}

impl View for DropDown {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(ControlKind::Picker, Record::new(), cx, self.handle.as_ref());
    let (options, chosen, choose) = (self.options, self.chosen, self.choose);

    tree::control::<Active, _>(
      id,
      contents::CHOICE,
      move || Choice {
        options: options(),
        chosen: chosen(),
      },
      choose
        .map(|choose| Box::new(move |choice: Choice| choose(choice.chosen)) as Box<dyn Fn(Choice)>),
    );

    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A checkbox node: its mark alone, its caption a text beside it that names it.
#[must_use]
pub(crate) struct Checkbox {
  record: Record,
  on: Rc<dyn Fn() -> bool>,
  mixed: Option<Rc<dyn Fn() -> bool>>,
  change: Option<Rc<dyn Fn(bool)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Checkbox {
  /// An unchecked checkbox, centered across the room its parent gives it.
  pub fn new() -> Self {
    let mut record = Record::new();

    record.set(keys::SELF_ALIGNMENT, Alignment::CENTER);

    Self {
      record,
      on: Rc::new(|| false),
      mixed: None,
      change: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// Whether it is checked, kept current.
  pub fn derive_on(mut self, on: impl Fn() -> bool + 'static) -> Self {
    self.on = Rc::new(on);

    self
  }

  /// Whether it is mixed, kept current.
  pub fn derive_mixed(mut self, mixed: impl Fn() -> bool + 'static) -> Self {
    self.mixed = Some(Rc::new(mixed));

    self
  }

  /// Runs `change` with the state the reader flipped it to.
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  enabled!();
  handle!();
  access!();
}

impl View for Checkbox {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(ControlKind::Checkbox, self.record, cx, self.handle.as_ref());
    let (on, change) = (self.on, self.change);

    tree::control::<Active, _>(
      id,
      contents::ON,
      move || on(),
      change.map(|change| Box::new(move |on| change(on)) as Box<dyn Fn(bool)>),
    );

    if let Some(mixed) = self.mixed {
      tree::bind::<Active, _>(id, contents::MIXED, move || mixed());
    }

    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A radio node: its mark alone, one choice of the group around it, its caption a text
/// beside it that names it.
#[must_use]
pub(crate) struct Radio {
  record: Record,
  on: Rc<dyn Fn() -> bool>,
  change: Option<Rc<dyn Fn(bool)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Radio {
  /// A radio that is off, centered across the room its parent gives it.
  pub fn new() -> Self {
    let mut record = Record::new();

    record.set(keys::SELF_ALIGNMENT, Alignment::CENTER);

    Self {
      record,
      on: Rc::new(|| false),
      change: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// Whether it is the group's choice, kept current.
  pub fn derive_on(mut self, on: impl Fn() -> bool + 'static) -> Self {
    self.on = Rc::new(on);

    self
  }

  /// Runs `change` when the reader chooses it (with `true`).
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  enabled!();
  handle!();
  access!();
}

impl View for Radio {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(ControlKind::Radio, self.record, cx, self.handle.as_ref());
    let (on, change) = (self.on, self.change);

    tree::control::<Active, _>(
      id,
      contents::ON,
      move || on(),
      change.map(|change| Box::new(move |on| change(on)) as Box<dyn Fn(bool)>),
    );

    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A column a reader takes for a group of a role (a radio group), named by a caption.
#[must_use]
pub(crate) struct Group {
  role: Role,
  record: Record,
  access: Access,
  children: Vec<crate::view::Child>,
}

impl Group {
  /// An empty group of `role`.
  pub fn new(role: Role) -> Self {
    Self {
      role,
      record: Record::new(),
      access: Access::default(),
      children: Vec::new(),
    }
  }

  properties! {
    /// The space between the members, in pixels.
    set_spacing, derive_spacing: SPACING, skene_foundation::Spacing;
  }

  access!();

  /// Appends a member, after the properties.
  pub fn add_child(mut self, child: impl View) -> Self {
    self.children.push(Box::new(move |cx| child.build(cx)));

    self
  }
}

impl View for Group {
  fn build(self, cx: &mut Cx) -> Node {
    let spec = NodeSpec::new(NodeKind::Layout(LayoutKind::Column)).with_role(self.role);
    let id = tree::create::<Active>(spec, &cx.scope);

    finish(id, self.record, cx, None);
    self.access.apply(id);
    crate::view::attach_children(id, self.children, cx);

    Node(id)
  }
}
