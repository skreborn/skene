//! Linux: GTK 4's controls, in the Linux build alone, and the native door.
//!
//! Each control is the toolkit's own, controlled: it shows what its `derive_` closure returns,
//! reports the reader's change to its `handle_` closure, and is brought back to the truth
//! after every report. Each is named for the reader by a caption placed anywhere
//! (`set_labelled_by`) or by a text (`set_accessible_name`), and carries the facts any control
//! may (`set_required`, `derive_invalid`, `set_described_by`).

use std::{cell::RefCell, rc::Rc};

pub use skene_foundation::seam::{
  content::{Content, ContentKey, ContentType, PropValue, Verdict},
  door::ControlSink,
};
use skene_foundation::{
  Alignment, Effect, NodeId,
  seam::{
    content::{Choice, Range, contents},
    node::{ControlKind, NodeKind, NodeSpec},
    record::{Record, keys},
    report::Report,
    tree,
  },
};
pub use skene_gui_linux::gtk;
use skene_gui_linux::gtk::prelude::*;

use crate::{
  Active, Signal,
  access::{Access, access},
  editing::{Editing, editing, enable, enabled},
  view::{Cx, Node, NodeHandle, View, finish, handle, properties},
};

/// GTK style classes: one (`"title-1"`), several (`["card", "activatable"]`), or none.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct StyleClasses(Rc<[Rc<str>]>);

impl From<&str> for StyleClasses {
  fn from(class: &str) -> Self {
    Self(Rc::from([Rc::from(class)]))
  }
}

impl<const N: usize> From<[&str; N]> for StyleClasses {
  fn from(classes: [&str; N]) -> Self {
    Self(classes.into_iter().map(Rc::from).collect())
  }
}

impl From<Vec<String>> for StyleClasses {
  fn from(classes: Vec<String>) -> Self {
    Self(classes.into_iter().map(Rc::from).collect())
  }
}

/// The Linux properties of a shared view (`set_linux_*`).
#[derive(Clone, Default)]
pub(crate) struct Extras {
  /// GTK style classes, beside Skene's own.
  pub(crate) style_classes: Option<Rc<dyn Fn() -> StyleClasses>>,
}

impl Extras {
  /// Applies the properties to `id`'s widget, kept current.
  pub(crate) fn apply(self, id: NodeId) {
    let (Some(classes), Some(widget)) = (self.style_classes, skene_gui_linux::widget(id)) else {
      return;
    };
    let shown = RefCell::new(StyleClasses::default());

    Effect::new(move || {
      let next = classes();
      let mut shown = shown.borrow_mut();

      for class in shown.0.iter().filter(|class| !next.0.contains(class)) {
        widget.remove_css_class(class);
      }

      for class in next.0.iter() {
        widget.add_css_class(class);
      }

      *shown = next;
    });
  }
}

/// Creates a control of `kind` in `cx`, its record `record` with the placement folded in, and
/// writes it into `handle`.
fn control(kind: ControlKind, record: Record, cx: &Cx, handle: Option<&NodeHandle>) -> NodeId {
  let id = tree::create::<Active>(NodeSpec::new(NodeKind::Control(kind)), &cx.scope);

  finish(id, record, cx, handle);

  id
}

/// A GTK switch (`GtkSwitch`).
#[must_use]
pub struct Switch {
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

  /// Whether the switch is on (`active`).
  pub fn set_on(self, on: bool) -> Self {
    self.derive_on(move || on)
  }

  /// Whether the switch is on (`active`), kept current.
  pub fn derive_on(mut self, on: impl Fn() -> bool + 'static) -> Self {
    self.on = Rc::new(on);

    self
  }

  /// Runs `change` with the state the reader flipped the switch to (`state-set`).
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

/// A GTK check button (`GtkCheckButton`): a box that is checked, unchecked or mixed, with its
/// own caption, which the reader clicks as part of it.
#[must_use]
pub struct CheckButton {
  caption: Option<Rc<dyn Fn() -> Rc<str>>>,
  on: Rc<dyn Fn() -> bool>,
  mixed: Option<Rc<dyn Fn() -> bool>>,
  change: Option<Rc<dyn Fn(bool)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Default for CheckButton {
  fn default() -> Self {
    Self::new()
  }
}

impl CheckButton {
  /// An unchecked check button without a caption.
  pub fn new() -> Self {
    Self {
      caption: None,
      on: Rc::new(|| false),
      mixed: None,
      change: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// The caption beside the box (`label`), which names it for a reader.
  pub fn set_caption(self, caption: impl Into<Rc<str>>) -> Self {
    let caption = caption.into();

    self.derive_caption(move || Rc::clone(&caption))
  }

  /// The caption beside the box (`label`), kept current.
  pub fn derive_caption<S: Into<Rc<str>>>(mut self, caption: impl Fn() -> S + 'static) -> Self {
    self.caption = Some(Rc::new(move || caption().into()));

    self
  }

  /// Whether the box is checked (`active`).
  pub fn set_on(self, on: bool) -> Self {
    self.derive_on(move || on)
  }

  /// Whether the box is checked (`active`), kept current.
  pub fn derive_on(mut self, on: impl Fn() -> bool + 'static) -> Self {
    self.on = Rc::new(on);

    self
  }

  /// Whether the box shows mixed (`inconsistent`): a box standing for several others that
  /// disagree.
  pub fn set_mixed(self, mixed: bool) -> Self {
    self.derive_mixed(move || mixed)
  }

  /// Whether the box shows mixed (`inconsistent`), kept current: a box standing for several
  /// others that disagree.
  pub fn derive_mixed(mut self, mixed: impl Fn() -> bool + 'static) -> Self {
    self.mixed = Some(Rc::new(mixed));

    self
  }

  /// Runs `change` with the state the reader checked the box to (`toggled`).
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.change = Some(Rc::new(change));

    self
  }

  /// Shows `on` and writes each check into it.
  pub fn bind_on(self, on: Signal<bool>) -> Self {
    self
      .derive_on(move || on.get())
      .handle_change(move |value| on.set(value))
  }

  enabled!();
  handle!();
  access!();
}

impl View for CheckButton {
  fn build(self, cx: &mut Cx) -> Node {
    let id = control(
      ControlKind::Checkbox,
      Record::new(),
      cx,
      self.handle.as_ref(),
    );
    let (on, change) = (self.on, self.change);

    if let Some(caption) = self.caption {
      tree::bind::<Active, _>(id, contents::CAPTION, move || caption());
    }

    if let Some(mixed) = self.mixed {
      tree::bind::<Active, _>(id, contents::MIXED, move || mixed());
    }

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

/// A GTK button (`GtkButton`).
#[must_use]
pub struct Button {
  caption: Rc<dyn Fn() -> String>,
  prominent: Option<Rc<dyn Fn() -> bool>>,
  activate: Option<Rc<dyn Fn()>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Button {
  /// A button showing `caption` (`label`).
  pub fn new(caption: impl Into<String>) -> Self {
    let caption = caption.into();

    Self::derive(move || caption.clone())
  }

  /// A button showing what `caption` returns (`label`), kept current.
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

  /// Shows the button as the window's primary action (the theme's `suggested-action` style
  /// class).
  pub fn set_prominent(self, prominent: bool) -> Self {
    self.derive_prominent(move || prominent)
  }

  /// Shows the button as the window's primary action (the theme's `suggested-action` style
  /// class), kept current.
  pub fn derive_prominent(mut self, prominent: impl Fn() -> bool + 'static) -> Self {
    self.prominent = Some(Rc::new(prominent));

    self
  }

  /// Runs `activate` when the button is activated (`clicked`): clicked, or pressed from the
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

    if let (Some(prominent), Some(widget)) = (self.prominent, skene_gui_linux::widget(id)) {
      Effect::new(move || {
        if prominent() {
          widget.add_css_class("suggested-action");
        } else {
          widget.remove_css_class("suggested-action");
        }
      });
    }

    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A GTK one-line entry (`GtkEntry`).
///
/// Each edit is reported before GTK applies it (`insert-text`, `delete-text`), as the whole
/// text it proposes; where the truth is that text GTK applies it natively, otherwise the truth
/// is shown as the smallest splice. A write from the signal is never `set_text`: an equal one
/// is skipped, one arriving while an input method composes waits for the commit, and the rest
/// are the smallest splice, the selection mapped through it.
#[must_use]
pub struct Entry {
  editing: Editing,
  placeholder: Option<Rc<dyn Fn() -> Rc<str>>>,
  secure: Option<Rc<dyn Fn() -> bool>>,
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
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  editing!(
    text: " (`text`)",
    change: " (`GtkText`'s `insert-text` and `delete-text`)",
    selection: " (`cursor-position` and `selection-bound`)",
    read_only: " (`editable`, negated)"
  );

  /// The text shown while the entry is empty (`placeholder-text`).
  pub fn set_placeholder(self, placeholder: impl Into<Rc<str>>) -> Self {
    let placeholder = placeholder.into();

    self.derive_placeholder(move || Rc::clone(&placeholder))
  }

  /// The text shown while the entry is empty (`placeholder-text`), kept current.
  pub fn derive_placeholder<S: Into<Rc<str>>>(
    mut self,
    placeholder: impl Fn() -> S + 'static,
  ) -> Self {
    self.placeholder = Some(Rc::new(move || placeholder().into()));

    self
  }

  /// Whether what is typed is hidden (`visibility`, negated).
  pub fn set_secure(self, secure: bool) -> Self {
    self.derive_secure(move || secure)
  }

  /// Whether what is typed is hidden, kept current.
  pub fn derive_secure(mut self, secure: impl Fn() -> bool + 'static) -> Self {
    self.secure = Some(Rc::new(secure));

    self
  }

  /// Runs `activate` when the reader presses `Enter` in the entry (`activate`).
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

    self.editing.apply(id);
    enable(id, self.enabled);
    self.access.apply(id);

    Node(id)
  }
}

/// A GTK multi-line text view (`GtkTextView`), wrapping its lines at word boundaries.
///
/// Each edit is reported before its buffer applies it (`insert-text`, `delete-range`), undo and
/// redo included, as the whole text it proposes; where the truth is that text GTK applies it
/// natively, otherwise the truth is shown as the smallest splice, the caret mapped through it.
/// Each move of the caret or the selection is reported (`mark-set`). Writes from the signal
/// follow the entry's rule.
#[must_use]
pub struct TextView {
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

  editing!(
    text: " (its buffer's `text`)",
    change: " (its buffer's `insert-text` and `delete-range`)",
    selection: " (its buffer's `insert` and `selection_bound` marks)",
    read_only: " (`editable`, negated)"
  );
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

/// A GTK scale (`GtkScale`): a value dragged along a range.
#[must_use]
pub struct Scale {
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

  /// Runs `change` with the value the reader moved the scale to (`change-value`), clamped to
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

/// A GTK dropdown (`GtkDropDown`): one choice of a list, shown closed.
#[must_use]
pub struct DropDown {
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

  /// The options' texts (its `GtkStringList` model).
  pub fn set_options<S: Into<Rc<str>>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Rc<[Rc<str>]> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.to_vec())
  }

  /// The options' texts (its `GtkStringList` model), kept current.
  pub fn derive_options<S: Into<Rc<str>>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.options = Rc::new(move || options().into_iter().map(Into::into).collect());

    self
  }

  /// The chosen option's index (`selected`), or none.
  pub fn set_chosen(self, chosen: Option<u32>) -> Self {
    self.derive_chosen(move || chosen)
  }

  /// The chosen option's index (`selected`), or none, kept current.
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

type Wire = Box<dyn FnOnce(NodeId)>;

/// The native door: any GTK widget as a leaf of a Skene layout, measured through GTK (its
/// minimum a floor) and placed by the engine as a built-in leaf is, its own alignment and
/// margins neutral.
///
/// A widget that holds a value joins the controlled contract through [`NativeView::set_sink`]
/// and [`NativeView::add_control`]: the library's [`ControlSink`] reads and writes the value by a
/// name of its own ([`ContentKey::door`]), the library reports what the reader did with
/// [`report_change`], and the truth is kept exactly as a built-in control's (the echo rule, the
/// equal-skip, the repair of a rejected change).
///
/// ```
/// use skene_gui::{platform::linux::{NativeView, gtk}, prelude::*};
///
/// fn calendar() -> impl View {
///   NativeView::new(gtk::Calendar::new()).set_accessible_name("Due date")
/// }
/// ```
#[must_use]
pub struct NativeView {
  widget: gtk::Widget,
  handle: Option<NodeHandle>,
  sink: Option<Rc<dyn ControlSink>>,
  controls: Vec<Wire>,
  access: Access,
}

impl NativeView {
  /// `widget`, adopted.
  pub fn new(widget: impl IsA<gtk::Widget>) -> Self {
    Self {
      widget: widget.upcast(),
      handle: None,
      sink: None,
      controls: Vec::new(),
      access: Access::default(),
    }
  }

  handle!();

  /// Reads and writes the widget's values by name: what [`NativeView::add_control`] keeps at
  /// the truth.
  pub fn set_sink(mut self, sink: impl ControlSink + 'static) -> Self {
    self.sink = Some(Rc::new(sink));

    self
  }

  /// Makes the widget's value `key` names controlled: the sink shows what `truth` returns,
  /// and a change the library reports ([`report_change`]) runs `change` and is answered with
  /// the truth.
  pub fn add_control<T: ContentType>(
    mut self,
    key: ContentKey<T>,
    truth: impl Fn() -> T + 'static,
    change: impl Fn(T) + 'static,
  ) -> Self {
    self.controls.push(Box::new(move |id| {
      tree::control::<Active, _>(id, key, truth, Some(Box::new(change)));
    }));

    self
  }

  access!();
}

impl View for NativeView {
  fn build(self, cx: &mut Cx) -> Node {
    let role = skene_gui_linux::role_of(&self.widget);
    let id = tree::adopt::<Active>(
      self.widget,
      NodeSpec::new(NodeKind::Native).with_role(role),
      &cx.scope,
      self.sink,
    );

    finish(id, Record::new(), cx, self.handle.as_ref());

    for control in self.controls {
      control(id);
    }

    self.access.apply(id);

    Node(id)
  }
}

/// Reports that the reader changed the value `key` names on `widget`, a widget a
/// [`NativeView`] adopted, to `proposed`, which the widget shows already: the view's `change`
/// runs, and where the truth is another value the sink is brought back to it. A change the
/// sink made itself, or that equals the truth, is not reported.
pub fn report_change<T: ContentType>(
  widget: &impl IsA<gtk::Widget>,
  key: ContentKey<T>,
  proposed: T,
) -> Verdict {
  let Some(id) = skene_gui_linux::nearest_node(widget.upcast_ref()) else {
    return Verdict::Ignore;
  };
  let content = key.content();
  let answer = tree::report::<Active>(
    id,
    Report::Change {
      content,
      proposed: proposed.into_value(),
      applied: true,
    },
  );

  match answer {
    skene_foundation::seam::report::Answer::Verdict(verdict) => {
      if matches!(verdict, Verdict::Replace(_))
        && let Some(controller) = tree::controller::<Active>(id, content)
      {
        controller.settle();
      }

      verdict
    }
    _ => Verdict::Ignore,
  }
}

/// Whether Skene is writing to the platform now: a library's widget notifies of its own
/// writes too, and [`report_change`] ignores them.
#[must_use]
pub fn is_writing() -> bool {
  skene_foundation::seam::content::is_writing()
}
