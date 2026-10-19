//! A control: focus, events, decoration, a label, parts and accessibility once, over the
//! behavior that says what its interaction is. Every built-in control is one, styled
//! (`Button` composes a `Control<Activate>`), and a library's drawn control is one too.

use std::rc::Rc;

use crate::{
  Arrangement, AxisAlignment, Memo, Spacing, State, Value,
  behavior::{self, Behavior, ControlHandle, ControlOps},
  layout::Axis,
  seam::{
    Realization,
    access::Shortcut,
    control::{ControlDispatch, Element, LabelPlacement, Part},
    node::{Interaction, LayoutKind, NodeKind, NodeSpec},
    record::{Declared, Record, keys},
    report::KeyEvent,
    tree::{self, Bubbling},
  },
  view::{
    Access, AnyKind, AnyView, Child, ChildOf, Cx, DecorationBase, HasRecord, MotionBase, Node,
    NodeHandle, Row, RunBase, RunWithEntries, Text, TextBase, View, Wraps, trait_properties,
    wraps_itself,
  },
};

/// A control's caption: a view built as its [`Part::Label`], with the name it supplies where
/// known. A text's label is named by its text.
#[must_use]
pub struct Label<R: Realization> {
  view: Child<R>,
  name: Option<Rc<dyn Fn() -> Rc<str>>>,
  empty: bool,
}

impl<R: Realization> Label<R> {
  /// A caption showing `view`, supplying no name of its own. It sits in its control's row or
  /// column, so a `Flexible` places it there.
  pub fn new(view: impl ChildOf<Row<R>, R>) -> Self {
    Self {
      view: Child::of::<Row<R>>(view),
      name: None,
      empty: false,
    }
  }

  /// Whether it is an empty text, which shows nothing: a control given one has no caption.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.empty
  }

  /// This caption, naming its control `name` where its control is named by its content (a
  /// button, an option), and finding it by `name` (type-ahead).
  pub fn set_name(self, name: impl Into<Rc<str>>) -> Self {
    let name = name.into();

    self.derive_name(move || Rc::clone(&name))
  }

  /// This caption, naming its control what `name` returns, kept current.
  pub fn derive_name<S: Into<Rc<str>>>(mut self, name: impl Fn() -> S + 'static) -> Self {
    self.name = Some(Rc::new(move || name().into()));

    self
  }

  /// The name it supplies as it reads now, tracking what it reads; none where its content
  /// names it and it states no name.
  #[must_use]
  pub fn name(&self) -> Option<Rc<str>> {
    self.name.as_ref().map(|name| name())
  }

  /// What supplies its name, to read apart from the caption: a picker's face showing the
  /// chosen option's name before its option is built.
  #[must_use]
  pub fn name_source(&self) -> Option<Rc<dyn Fn() -> Rc<str>>> {
    self.name.clone()
  }

  /// This caption with its view placed in what `wrap` makes of it, its name kept: a styled
  /// control placing a caption it was given (a `Flexible` taking the room its row leaves).
  pub fn wrap_view<V: ChildOf<Row<R>, R>>(
    self,
    wrap: impl FnOnce(AnyView<R>) -> V + 'static,
  ) -> Self {
    let view = self.view;

    Self {
      view: Child::of::<Row<R>>(wrap(AnyView(view))),
      ..self
    }
  }
}

impl<R: Realization> From<&str> for Label<R> {
  fn from(text: &str) -> Self {
    Self::from(Rc::<str>::from(text))
  }
}

impl<R: Realization> From<String> for Label<R> {
  fn from(text: String) -> Self {
    Self::from(Rc::<str>::from(text))
  }
}

impl<R: Realization> From<Rc<str>> for Label<R> {
  fn from(text: Rc<str>) -> Self {
    let empty = text.is_empty();

    Self {
      empty,
      ..Self::new(Text::<R>::new(text.to_string())).set_name(text)
    }
  }
}

/// Records its one child as `part` of the control it is built inside: the child's
/// states are the control's, and a reader skips a part that is decoration
/// ([`Part::is_presentational`]). It realizes nothing of its own; without a child it shows
/// nothing.
#[must_use]
pub struct AsPart<R: Realization> {
  part: Part,
  child: Option<Child<R>>,
}

wraps_itself!(AsPart<R> where R: Realization);

/// What an [`AsPart`] carries: its part and its child.
pub trait AsPartBase<R: Realization>: Wraps<Inner = AsPart<R>> {
  /// A wrapper recording its child as `part`.
  fn new(part: Part) -> Self {
    Self::wrap(AsPart { part, child: None })
  }

  /// The child, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(Child::view(child));

    self
  }
}

impl<R: Realization> AsPartBase<R> for AsPart<R> {}

impl<R: Realization> AnyKind for AsPart<R> {}

impl<R: Realization> View<R> for AsPart<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    cx.set_part(self.part);

    match self.child {
      Some(child) => child.build(cx),
      None => None::<Text<R>>.build(cx),
    }
  }
}

/// What hears each key a control's behavior heard, deciding nothing.
type KeyObserver = Rc<dyn Fn(&KeyEvent)>;

/// Everything a control carries beside its behavior, its parts and its body: its root's
/// record, its label, its flags, its accessibility, its observers and its handle. What the
/// members of [`ControlMembersBase`] write.
#[must_use]
pub struct Members<R: Realization> {
  record: Record,
  label: Option<Label<R>>,
  placement: Option<LabelPlacement>,
  disabled: Option<Rc<dyn Fn() -> bool>>,
  access: Access,
  focus: Option<Rc<dyn Fn(bool)>>,
  key: Option<KeyObserver>,
  handle: Option<NodeHandle>,
  autofocus: bool,
}

impl<R: Realization> Default for Members<R> {
  fn default() -> Self {
    Self {
      record: Record::new(),
      label: None,
      placement: None,
      disabled: None,
      access: Access::default(),
      focus: None,
      key: None,
      handle: None,
      autofocus: false,
    }
  }
}

impl<R: Realization> Members<R> {
  /// The root's properties, as its setters gave them.
  #[must_use]
  pub const fn record(&self) -> &Record {
    &self.record
  }
}

/// A view whose root is a control's: what [`ControlMembersBase`] writes its members into.
pub trait HasMembers<R: Realization>: HasRecord {
  /// The control's members.
  fn members_mut(&mut self) -> &mut Members<R>;
}

/// What every control carries: its row's spacing and alignments, its caption, whether it is
/// disabled, its accessibility, its shortcut, its observers, its handle, and whether it asks
/// for focus as its window or popup opens.
pub trait ControlMembersBase<R: Realization>: Wraps<Inner: HasMembers<R>> {
  trait_properties! {
    /// Space between the label, the parts and the body, in pixels.
    set_spacing, derive_spacing: SPACING, Spacing;
    /// How the label, the parts and the body are distributed along the control.
    set_arrangement, derive_arrangement: ARRANGEMENT, Arrangement;
    /// Where the label, the parts and the body sit across the control.
    set_item_alignment, derive_item_alignment: ITEM_ALIGNMENT, AxisAlignment;
  }

  /// The caption: a text (named by its text) or any view ([`Label`]); an empty text leaves the
  /// control without one, named by [`ControlMembersBase::set_accessible_name`] or a caption
  /// placed anywhere.
  fn set_label(mut self, label: impl Into<Label<R>>) -> Self {
    let label = label.into();

    self.inner_mut().members_mut().label = (!label.is_empty()).then_some(label);

    self
  }

  /// Where the caption sits, over the behavior's own choice.
  fn set_label_placement(mut self, placement: LabelPlacement) -> Self {
    self.inner_mut().members_mut().placement = Some(placement);

    self
  }

  /// Whether the control is present but not operable: it takes no focus and no input, and it
  /// shows dimmed unless its style says otherwise. A control inside a disabled control is
  /// disabled with it, as a fieldset disables the controls inside it.
  fn set_disabled(self, disabled: bool) -> Self {
    self.derive_disabled(move || disabled)
  }

  /// Whether the control is present but not operable, kept current.
  fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.inner_mut().members_mut().disabled = Some(Rc::new(disabled));

    self
  }

  /// Whether a value is required.
  fn set_required(self, required: bool) -> Self {
    self.derive_required(move || required)
  }

  /// Whether a value is required, kept current.
  fn derive_required(mut self, required: impl Fn() -> bool + 'static) -> Self {
    self.inner_mut().members_mut().access.required = Some(Rc::new(required));

    self
  }

  /// Whether the value is invalid; the control is styled under `State::Invalid` while it is.
  fn set_invalid(self, invalid: bool) -> Self {
    self.derive_invalid(move || invalid)
  }

  /// Whether the value is invalid, kept current.
  fn derive_invalid(mut self, invalid: impl Fn() -> bool + 'static) -> Self {
    self.inner_mut().members_mut().access.invalid = Some(Rc::new(invalid));

    self
  }

  /// Names the control for a reader where no caption names it.
  fn set_accessible_name(self, name: impl Into<Rc<str>>) -> Self {
    let name = name.into();

    self.derive_accessible_name(move || Rc::clone(&name))
  }

  /// Names the control for a reader where no caption names it, kept current.
  fn derive_accessible_name<S: Into<Rc<str>>>(mut self, name: impl Fn() -> S + 'static) -> Self {
    self.inner_mut().members_mut().access.name = Some(Rc::new(move || name().into()));

    self
  }

  /// Names the control by the text of the node `caption` captured, placed anywhere.
  fn set_labelled_by(mut self, caption: &NodeHandle) -> Self {
    self.inner_mut().members_mut().access.labelled_by = vec![*caption];

    self
  }

  /// Describes the control for a reader.
  fn set_accessible_description(self, description: impl Into<Rc<str>>) -> Self {
    let description = description.into();

    self.derive_accessible_description(move || Rc::clone(&description))
  }

  /// Describes the control for a reader, kept current.
  fn derive_accessible_description<S: Into<Rc<str>>>(
    mut self,
    description: impl Fn() -> S + 'static,
  ) -> Self {
    self.inner_mut().members_mut().access.description = Some(Rc::new(move || description().into()));

    self
  }

  /// Describes the control by the text of the node `description` captured, placed anywhere.
  fn set_described_by(mut self, description: &NodeHandle) -> Self {
    self.inner_mut().members_mut().access.described_by = vec![*description];

    self
  }

  /// Clicks the control when `shortcut` (`Shortcut::primary('s')`: Control+S on Linux and
  /// Windows, Command+S on macOS) is pressed wherever focus is in its window, unless the
  /// focused control uses the key itself; a reader reads it as the control's key binding.
  fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.inner_mut().members_mut().access.shortcut = Some(shortcut);

    self
  }

  /// Runs `focus` when focus enters the control (`true`) or leaves it (`false`).
  fn handle_focus(mut self, focus: impl Fn(bool) + 'static) -> Self {
    self.inner_mut().members_mut().focus = Some(Rc::new(focus));

    self
  }

  /// Runs `key` with each key while focus is on the control, after its behavior; it never
  /// decides what happens to the key.
  fn handle_key(mut self, key: impl Fn(&KeyEvent) + 'static) -> Self {
    self.inner_mut().members_mut().key = Some(Rc::new(key));

    self
  }

  /// Writes the control's node into `handle` once built.
  fn set_handle(mut self, handle: &NodeHandle) -> Self {
    self.inner_mut().members_mut().handle = Some(*handle);

    self
  }

  /// Whether the control asks for focus when its window or its popup opens: the first control
  /// in the scene's order that asks and takes focus then gets it, its focus shown until a
  /// pointer press. It is read once, as the control is built; a window or a popup none of
  /// whose controls asks opens with focus where it was (a window with none).
  fn set_autofocus(mut self, autofocus: bool) -> Self {
    self.inner_mut().members_mut().autofocus = autofocus;

    self
  }
}

/// A control: focus, events, decoration, a label, parts and accessibility once, over the
/// behavior that says what the interaction is ([`Behavior`]). A library's drawn control is
/// one.
///
/// Its node is a row (or a column, [`ControlBase::set_axis`]) of its label, its parts and its
/// body, or the realization's own text view where its behavior's element is one. Every node
/// built inside it resolves every state but `FocusWithin` against the control (its hover,
/// focus and press, its caption's beside it, and its own states): a part styled
/// `when(State::Checked, ..)` follows the control's check, and one styled
/// `when(State::Hovered, ..)` highlights while the pointer is over the control or its
/// caption. A box that acts inside it keeps its own hover, focus and press. A disabled
/// control, or one inside a disabled control, is drawn at half opacity unless its style says
/// otherwise, takes no focus and swallows its input.
#[must_use]
pub struct Control<B: Behavior, R: Realization> {
  /// What its interaction is.
  behavior: B,
  /// The axis its label, parts and body lie along.
  axis: Axis,
  /// Everything it carries beside its behavior, its parts and its body.
  members: Members<R>,
  /// Its parts, in order.
  parts: Vec<Child<R>>,
  /// Its body.
  child: Option<Child<R>>,
}

wraps_itself!(Control<B, R> where B: Behavior, R: Realization);

impl<B: Behavior, R: Realization> HasRecord for Control<B, R> {
  #[inline(always)]
  fn record_mut(&mut self) -> &mut Record {
    &mut self.members.record
  }
}

impl<B: Behavior, R: Realization> HasMembers<R> for Control<B, R> {
  #[inline(always)]
  fn members_mut(&mut self) -> &mut Members<R> {
    &mut self.members
  }
}

/// What a [`Control`] carries beside every control's members: its behavior, its axis, its
/// parts and its body.
pub trait ControlBase<B: Behavior, R: Realization>: Wraps<Inner = Control<B, R>> {
  /// A control over `behavior`, laid out along a row, with no label, part or body.
  fn new(behavior: B) -> Self {
    Self::wrap(Control {
      behavior,
      axis: Axis::Horizontal,
      members: Members::default(),
      parts: Vec::new(),
      child: None,
    })
  }

  /// Its behavior.
  fn behavior(&self) -> &B {
    &self.inner().behavior
  }

  /// This control with its behavior changed by `change`.
  fn map_behavior(self, change: impl FnOnce(B) -> B) -> Self {
    self.map_inner(|mut control| {
      control.behavior = change(control.behavior);

      control
    })
  }

  /// Lays the label, the parts and the body along `axis`; horizontal where unset.
  fn set_axis(mut self, axis: Axis) -> Self {
    self.inner_mut().axis = axis;

    self
  }

  /// Appends `view` as the control's `part`, after its leading label.
  fn add_part(mut self, part: Part, view: impl ChildOf<Control<B, R>, R>) -> Self {
    self
      .inner_mut()
      .parts
      .push(Child::new(move |cx: &mut Cx<R>| {
        cx.set_part(part);

        view.build_child(cx)
      }));

    self
  }

  /// The body, after the parts; last.
  fn set_child(mut self, child: impl ChildOf<Control<B, R>, R>) -> Self {
    self.inner_mut().child = Some(Child::of::<Control<B, R>>(child));

    self
  }
}

impl<B: Behavior, R: Realization> Control<B, R> {
  /// This control over `behavior` in place of its own, everything else it carries kept: a
  /// styled member given its behavior by its group as the group builds it.
  pub fn with_behavior<C: Behavior>(self, behavior: C) -> Control<C, R> {
    Control {
      behavior,
      axis: self.axis,
      members: self.members,
      parts: self.parts,
      child: self.child,
    }
  }
}

impl<B: Behavior, R: Realization> ControlBase<B, R> for Control<B, R> {}

impl<B: Behavior, R: Realization> ControlMembersBase<R> for Control<B, R> {}

impl<B: Behavior, R: Realization> DecorationBase for Control<B, R> {}

impl<B: Behavior, R: Realization> RunBase for Control<B, R> {}

impl<B: Behavior, R: Realization> RunWithEntries for Control<B, R> {}

impl<B: Behavior, R: Realization> MotionBase for Control<B, R> {}

impl<B: Behavior, R: Realization> AnyKind for Control<B, R> {}

impl<B: Behavior, R: Realization> View<R> for Control<B, R> {
  #[expect(clippy::too_many_lines, reason = "a control's every step, in order")]
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      behavior,
      axis,
      members,
      parts,
      child,
    } = self;
    let behavior = Rc::new(behavior);
    let role = behavior.role();
    let placement = members
      .placement
      .unwrap_or_else(|| behavior.label_placement());
    let element = behavior.element();
    let kind = match element {
      Element::Box => NodeKind::Layout(match axis {
        Axis::Horizontal => LayoutKind::Row,
        Axis::Vertical => LayoutKind::Column,
      }),
      Element::Editable { lines } => NodeKind::Editable {
        multiline: lines > 1,
      },
    };
    // A text view holds no caption, so one placed leading or trailing sits beside it, before
    // or after it among its siblings.
    let placement = match (element, placement) {
      (Element::Editable { .. }, LabelPlacement::Leading) => LabelPlacement::Beside,
      _ => placement,
    };
    let after =
      matches!(element, Element::Editable { .. }) && placement == LabelPlacement::Trailing;
    let spec = NodeSpec::new(kind)
      .with_role(role)
      .with_interaction(Interaction::Control {
        drag: behavior.drag_axes(),
      });
    let beside = (placement == LabelPlacement::Beside || after) && members.label.is_some();
    let segment = beside.then(|| cx.create(NodeSpec::new(NodeKind::Segment)));
    let id = cx.create(spec);
    let mut record = Record::new();

    record.set(
      keys::OPACITY,
      Value::from(1.).when(State::Disabled, Value::from(0.5)),
    );
    record.fold(&members.record);

    let opacity = record
      .entries()
      .find(|(prop, _)| *prop == keys::OPACITY.prop())
      .map(|(_, opacity)| opacity.clone());

    cx.finish(id, record, members.handle.as_ref());

    let disabled = members.disabled.clone();
    let disabled = Memo::new(move || {
      disabled.as_ref().is_some_and(|disabled| disabled()) || tree::enclosing_disabled::<R>(id)
    });

    if members.autofocus {
      tree::set_autofocus::<R>(id);
    }

    let invalid = members.access.invalid.clone();
    let label_name = members.label.as_ref().and_then(|label| label.name.clone());
    let named_by_content = role.is_named_by_content();
    let handle = {
      let (access_name, label_name) = (members.access.name.clone(), label_name.clone());

      ControlHandle::new(
        id,
        ControlOps::of::<R>(),
        move || {
          access_name
            .as_ref()
            .or(label_name.as_ref())
            .map(|name| name())
        },
        move || disabled.get(),
      )
    };

    {
      let (behavior, handle, invalid) = (Rc::clone(&behavior), handle.clone(), invalid.clone());

      tree::set_control_state::<R>(id, move || {
        handle.track_refresh();

        let mut state = behavior.state();

        state.disabled |= disabled.get();
        state.invalid |= invalid.as_ref().is_some_and(|invalid| invalid());

        state
      });
    }

    let caption = NodeHandle::new();

    {
      let (behavior, handle) = (Rc::clone(&behavior), handle.clone());
      let access = members.access.clone();
      let has_label = members.label.is_some();

      tree::add_access::<R>(id, move || {
        handle.track_refresh();

        let nodes = |handles: &[NodeHandle]| handles.iter().filter_map(NodeHandle::node).collect();
        let mut facts = behavior.access();
        let disabled = disabled.get();

        facts.disabled = disabled;
        facts.focusable = !disabled && behavior.focusable();
        facts.shortcut = access.shortcut;
        facts.required |= access.required.as_ref().is_some_and(|required| required());
        facts.invalid |= access.invalid.as_ref().is_some_and(|invalid| invalid());

        if let Some(name) = &access.name {
          facts.name = Some(name());
        } else if !access.labelled_by.is_empty() {
          facts.labelled_by = nodes(&access.labelled_by);
        } else if named_by_content {
          facts.name = label_name.as_ref().map(|name| name());
        } else if has_label {
          facts.labelled_by = caption.node().into_iter().collect();
        }

        facts.description = access.description.as_ref().map(|description| description());
        facts.described_by = nodes(&access.described_by);

        facts
      });
    }

    {
      let control = |behavior: &Rc<B>| (Rc::clone(behavior), handle.clone());
      let ((key_behavior, key_handle), (pointer_behavior, pointer_handle)) =
        (control(&behavior), control(&behavior));
      let ((click_behavior, click_handle), (access_behavior, access_handle)) =
        (control(&behavior), control(&behavior));
      let (blur_behavior, blur_handle) = control(&behavior);

      tree::handle_control::<R>(
        id,
        ControlDispatch {
          key: Rc::new(move |event| key_behavior.on_key(event, &key_handle)),
          pointer: Rc::new(move |event| pointer_behavior.on_pointer(event, &pointer_handle)),
          click: Rc::new(move || click_behavior.on_click(&click_handle)),
          access: Rc::new(move |request| access_behavior.on_access(request, &access_handle)),
          blur: Rc::new(move || blur_behavior.on_blur(&blur_handle)),
          disabled: Rc::new(move || disabled.get()),
        },
      );
    }

    if let Some(focus) = members.focus {
      tree::handle_focus::<R>(id, move |focused| focus(focused));
    }

    if let Some(key) = members.key {
      tree::handle_key::<R, _>(id, move |event| {
        key(event);

        Bubbling::Continue
      });
    }

    behavior.attach(&handle);

    let mut inner = cx.child();

    inner.set_control(id);

    let label = members.label.map(|label| {
      let view = label.view;

      Child::new(move |cx: &mut Cx<R>| {
        cx.set_part(Part::Label);
        cx.set_presentational(named_by_content);

        if beside {
          cx.set_interaction(Interaction::Answering);
        }

        let node = view.build(cx);

        caption.set(node.id());

        node
      })
    });
    let (mut leading, mut trailing, mut besides) = (None, None, None);

    match placement {
      _ if beside => besides = label,
      LabelPlacement::Leading | LabelPlacement::Beside => leading = label,
      LabelPlacement::Trailing => trailing = label,
    }

    let children: Vec<Child<R>> = leading
      .into_iter()
      .chain(parts)
      .chain(child)
      .chain(trailing)
      .collect();

    inner.attach_children(id, children);
    behavior.mounted(&handle);

    match (segment, besides) {
      (Some(segment), Some(label)) => {
        let label = label.build(&mut inner.child());

        if let Some(opacity) = opacity {
          share_opacity::<R>(label.id(), opacity);
        }

        let (first, second) = if after {
          (id, label.id())
        } else {
          (label.id(), id)
        };

        tree::attach::<R>(segment, first, 0);
        tree::attach::<R>(segment, second, 1);

        Node::new(segment)
      }
      _ => Node::new(id),
    }
  }
}

/// Draws a caption beside its control at the control's opacity, so it dims with a disabled
/// control, unless its own style sets one.
fn share_opacity<R: Realization>(caption: crate::NodeId, opacity: Declared) {
  if let Some(record) = tree::record::<R>(caption)
    && !record.has(keys::OPACITY.prop())
  {
    let mut record = (*record).clone();

    record.put(keys::OPACITY.prop(), opacity);
    tree::set_record::<R>(caption, record);
  }
}

/// A button with no style of its own: a [`Control`] over [`behavior::Activate`].
pub type BasicButton<R> = Control<behavior::Activate, R>;
/// A switch or a checkbox with no style of its own: a [`Control`] over [`behavior::Toggle`].
pub type BasicCheckbox<R> = Control<behavior::Toggle, R>;
/// A slider with no style of its own: a [`Control`] over [`behavior::Slider`].
pub type BasicSlider<R> = Control<behavior::Slider, R>;
/// A radio group holding a `T`, with no style of its own: a [`Control`] over
/// [`behavior::RadioGroup`].
pub type BasicRadioGroup<T, R> = Control<behavior::RadioGroup<T>, R>;
/// A radio carrying a `T`, with no style of its own: a [`Control`] over [`behavior::Radio`].
pub type BasicRadio<T, R> = Control<behavior::Radio<T>, R>;
/// A list box choosing `C`, with no style of its own: a [`Control`] over
/// [`behavior::Listbox`].
pub type BasicListbox<C, R> = Control<behavior::Listbox<C>, R>;
/// A list box's option, with no style of its own: a [`Control`] over
/// [`behavior::ListboxOption`].
pub type BasicListboxOption<C, R> = Control<behavior::ListboxOption<C>, R>;
/// A picker's face holding a `T`, with no style of its own: a [`Control`] over a
/// [`behavior::Popup`], composed by its user with an `Overlay` holding a [`BasicListbox`] of
/// [`BasicListboxOption`]s over the same roster.
pub type BasicPicker<T, R> = Control<behavior::Popup<T>, R>;
/// A text field with no style of its own: a [`Control`] over a one-line
/// [`behavior::TextEntry`].
pub type BasicTextField<R> = Control<behavior::TextEntry, R>;
/// A text editor with no style of its own: a [`Control`] over a [`behavior::TextEntry`] of
/// several lines.
pub type BasicTextEditor<R> = Control<behavior::TextEntry, R>;
