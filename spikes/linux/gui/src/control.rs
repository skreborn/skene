//! A control: focus, events, decoration, a label, parts and accessibility once, over the
//! behavior that says what its interaction is. Every built-in control is one, styled
//! (`Button` composes a `Control<Activate>`), and a library's drawn control is one too.

use std::rc::Rc;

use skene_foundation::{
  Memo, State, Value,
  behavior::{self, Behavior, ControlHandle, ControlOps},
  layout::Axis,
  seam::{
    control::{ControlDispatch, Element, LabelPlacement, Part},
    node::{Interaction, LayoutKind, NodeKind, NodeSpec},
    record::{Record, keys},
    report::KeyEvent,
    tree::{self, Bubbling},
  },
};

use crate::{
  Active, Row, Text,
  access::Access,
  view::{Child, ChildOf, Cx, Node, NodeHandle, View, attach_children, child_of, finish},
};

/// The realization's operations a control's behavior acts through, for this build.
const OPS: ControlOps = ControlOps::of::<Active>();

/// A control's caption: a view built as its [`Part::Label`], with the name it supplies where
/// known. A text's label is named by its text.
#[must_use]
pub struct Label {
  view: Child,
  name: Option<Rc<dyn Fn() -> Rc<str>>>,
}

impl Label {
  /// A caption showing `view`, supplying no name of its own. It sits in its control's row or
  /// column, so a `Flexible` places it there.
  pub fn new(view: impl ChildOf<Row>) -> Self {
    Self {
      view: child_of::<Row>(view),
      name: None,
    }
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
}

impl From<&str> for Label {
  fn from(text: &str) -> Self {
    Self::from(text.to_owned())
  }
}

impl From<String> for Label {
  fn from(text: String) -> Self {
    let name: Rc<str> = Rc::from(text.as_str());

    Self::new(Text::new(text)).set_name(name)
  }
}

impl From<Rc<str>> for Label {
  fn from(text: Rc<str>) -> Self {
    Self::new(Text::new(text.to_string())).set_name(text)
  }
}

/// Records its one child as `part` of the control it is built inside: the child's
/// control-scoped states are the control's, and a reader skips a part that is decoration
/// ([`Part::is_presentational`]). It realizes nothing of its own; without a child it shows
/// nothing.
#[must_use]
pub struct AsPart {
  part: Part,
  child: Option<Child>,
}

impl AsPart {
  /// A wrapper recording its child as `part`.
  pub const fn new(part: Part) -> Self {
    Self { part, child: None }
  }

  /// The child, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(Box::new(move |cx| child.build(cx)));

    self
  }
}

impl View for AsPart {
  fn build(self, cx: &mut Cx) -> Node {
    cx.part = Some(self.part);
    cx.presentational = self.part.is_presentational();

    match self.child {
      Some(child) => child(cx),
      None => None::<Text>.build(cx),
    }
  }
}

/// What hears each key a control's behavior heard, deciding nothing.
type KeyObserver = Rc<dyn Fn(&KeyEvent)>;

/// Everything a control carries beside its behavior, its parts and its body: its root's
/// record, its label, its flags, its accessibility, its observers and its handle.
#[derive(Default)]
pub(crate) struct Members {
  /// The root's properties, as its setters gave them, over the control's own defaults.
  pub(crate) record: Record,
  /// The caption.
  pub(crate) label: Option<Label>,
  /// Where the caption sits, the behavior's choice where unset.
  pub(crate) placement: Option<LabelPlacement>,
  /// Whether it is disabled.
  pub(crate) disabled: Option<Rc<dyn Fn() -> bool>>,
  /// Its accessibility facts beyond its behavior's.
  pub(crate) access: Access,
  /// What runs when focus enters or leaves it.
  pub(crate) focus: Option<Rc<dyn Fn(bool)>>,
  /// What hears each key after its behavior.
  pub(crate) key: Option<KeyObserver>,
  /// Where its node is written once built.
  pub(crate) handle: Option<NodeHandle>,
}

/// A property's `set_` and `derive_` setters on a control's root record, reached through its
/// [`Members`] (`members_mut`).
macro_rules! member_properties {
  ($($(#[doc = $doc:literal])* $set:ident, $derive:ident: $key:ident, $ty:ty;)*) => {$(
    $(#[doc = $doc])*
    pub fn $set(mut self, value: impl Into<::skene_foundation::Value<$ty>>) -> Self {
      self
        .members_mut()
        .record
        .set(::skene_foundation::seam::record::keys::$key, value);

      self
    }

    $(#[doc = $doc])*
    ///
    /// Derived: recomputed whenever a signal the closure reads changes.
    pub fn $derive<V: Into<::skene_foundation::Value<$ty>>>(
      mut self,
      value: impl Fn() -> V + 'static,
    ) -> Self {
      self.members_mut().record.set(
        ::skene_foundation::seam::record::keys::$key,
        ::skene_foundation::Value::derived(value),
      );

      self
    }
  )*};
}

pub(crate) use member_properties;

/// Every member a control carries (its root's decoration, sizing and inherited text, its
/// label, its flags, its accessibility, its observers, its handle), written on a type that
/// reaches its [`Members`] through `$path`: `Control<B>` and every built-in that composes one.
macro_rules! control_members {
  ($($path:ident).+) => {
    /// Its members, where its setters write.
    fn members_mut(&mut self) -> &mut $crate::control::Members {
      &mut self.$($path).+
    }

    $crate::control::member_properties! {
      /// Space between the control's edges and what it holds, in pixels.
      set_padding, derive_padding: PADDING, ::skene_foundation::Inset;
      /// The fill behind the control: a color or a gradient.
      set_background, derive_background: BACKGROUND, ::skene_foundation::Fill;
      /// The corners' radii, in pixels.
      set_corner_radius, derive_corner_radius: CORNER_RADIUS, ::skene_foundation::Corners;
      /// A line around the control, inside its edges.
      set_border, derive_border: BORDER, ::skene_foundation::Border;
      /// The fill of the line around the control (a color or a gradient), its text color
      /// where unset.
      set_border_color, derive_border_color: BORDER_COLOR, ::skene_foundation::Fill;
      /// The shadows the control casts.
      set_shadow, derive_shadow: SHADOW, ::skene_foundation::Shadows;
      /// The control's opacity, 0 to 1; half where disabled unless set.
      set_opacity, derive_opacity: OPACITY, f32;
      /// A line around the control that takes no room (a focus ring).
      set_ring, derive_ring: RING, ::skene_foundation::Ring;
      /// The ring's color, the control's text color where unset.
      set_ring_color, derive_ring_color: RING_COLOR, ::skene_foundation::Color;
      /// How far the control is drawn from where it was laid out, taking no room.
      set_offset, derive_offset: OFFSET, ::skene_foundation::Offset;
      /// The width.
      set_width, derive_width: WIDTH, ::skene_foundation::Dimension;
      /// The height.
      set_height, derive_height: HEIGHT, ::skene_foundation::Dimension;
      /// The least width.
      set_min_width, derive_min_width: MIN_WIDTH, ::skene_foundation::Dimension;
      /// The least height.
      set_min_height, derive_min_height: MIN_HEIGHT, ::skene_foundation::Dimension;
      /// The greatest width.
      set_max_width, derive_max_width: MAX_WIDTH, ::skene_foundation::Dimension;
      /// The greatest height.
      set_max_height, derive_max_height: MAX_HEIGHT, ::skene_foundation::Dimension;
      /// Space between the label, the parts and the body, in pixels.
      set_spacing, derive_spacing: SPACING, ::skene_foundation::Spacing;
      /// How the label, the parts and the body are distributed along the control.
      set_arrangement, derive_arrangement: ARRANGEMENT, ::skene_foundation::Arrangement;
      /// Where the label, the parts and the body sit across the control.
      set_item_alignment, derive_item_alignment: ITEM_ALIGNMENT, ::skene_foundation::AxisAlignment;
      /// The color of the text inside, which every text and mark beneath inherits.
      set_color, derive_color: COLOR, ::skene_foundation::Color;
      /// The font's size inside, in pixels.
      set_font_size, derive_font_size: FONT_SIZE, f32;
      /// The font's weight inside.
      set_font_weight, derive_font_weight: FONT_WEIGHT, ::skene_foundation::FontWeight;
      /// Upright or slanted inside.
      set_font_style, derive_font_style: FONT_STYLE, ::skene_foundation::FontStyle;
      /// The typeface inside.
      set_font_family, derive_font_family: FONT_FAMILY, ::skene_foundation::FontFamily;
      /// The height of a line inside, as a multiple of the font's size.
      set_line_height, derive_line_height: LINE_HEIGHT, f32;
      /// Extra space between letters inside, in pixels.
      set_letter_spacing, derive_letter_spacing: LETTER_SPACING, f32;
      /// A text entry's caret inside, the text's color where unset.
      set_caret_color, derive_caret_color: CARET_COLOR, ::skene_foundation::Color;
      /// The fill behind a text entry's selection inside.
      set_selection_color, derive_selection_color: SELECTION_COLOR, ::skene_foundation::Color;
      /// An empty text entry's placeholder inside.
      set_placeholder_color, derive_placeholder_color: PLACEHOLDER_COLOR, ::skene_foundation::Color;
    }

    /// Animates changes of a property of the control's own look as `transition` says; one of
    /// the same property is replaced.
    pub fn add_transition(mut self, transition: ::skene_foundation::seam::record::Transition) -> Self {
      self.members_mut().record.add_transition(transition);

      self
    }

    /// Plays `animation`'s keyframes over the control's own look from when the control first
    /// takes it, after the animations added before.
    pub fn add_animation(mut self, animation: ::skene_foundation::seam::record::Animation) -> Self {
      self.members_mut().record.add_animation(animation);

      self
    }

    /// The caption: a text (named by its text) or any view ([`crate::Label`]).
    pub fn set_label(mut self, label: impl Into<$crate::Label>) -> Self {
      self.$($path).+.label = Some(label.into());

      self
    }

    /// Where the caption sits, over the behavior's own choice.
    pub const fn set_label_placement(
      mut self,
      placement: ::skene_foundation::seam::control::LabelPlacement,
    ) -> Self {
      self.$($path).+.placement = Some(placement);

      self
    }

    /// Whether the control is present but not operable: it takes no focus and no input, and
    /// it shows dimmed unless its style says otherwise.
    pub fn set_disabled(self, disabled: bool) -> Self {
      self.derive_disabled(move || disabled)
    }

    /// Whether the control is present but not operable, kept current.
    pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
      self.$($path).+.disabled = Some(::std::rc::Rc::new(disabled));

      self
    }

    /// Whether a value is required.
    pub fn set_required(self, required: bool) -> Self {
      self.derive_required(move || required)
    }

    /// Whether a value is required, kept current.
    pub fn derive_required(mut self, required: impl Fn() -> bool + 'static) -> Self {
      self.$($path).+.access.required = Some(::std::rc::Rc::new(required));

      self
    }

    /// Whether the value is invalid; the control is styled under `State::Invalid` while it is.
    pub fn set_invalid(self, invalid: bool) -> Self {
      self.derive_invalid(move || invalid)
    }

    /// Whether the value is invalid, kept current.
    pub fn derive_invalid(mut self, invalid: impl Fn() -> bool + 'static) -> Self {
      self.$($path).+.access.invalid = Some(::std::rc::Rc::new(invalid));

      self
    }

    /// Names the control for a reader where no caption names it.
    pub fn set_accessible_name(self, name: impl Into<::std::rc::Rc<str>>) -> Self {
      let name = name.into();

      self.derive_accessible_name(move || ::std::rc::Rc::clone(&name))
    }

    /// Names the control for a reader where no caption names it, kept current.
    pub fn derive_accessible_name<S: Into<::std::rc::Rc<str>>>(
      mut self,
      name: impl Fn() -> S + 'static,
    ) -> Self {
      self.$($path).+.access.name = Some(::std::rc::Rc::new(move || name().into()));

      self
    }

    /// Names the control by the text of the node `caption` captured, placed anywhere.
    pub fn set_labelled_by(mut self, caption: &$crate::NodeHandle) -> Self {
      self.$($path).+.access.labelled_by = vec![*caption];

      self
    }

    /// Describes the control for a reader.
    pub fn set_accessible_description(self, description: impl Into<::std::rc::Rc<str>>) -> Self {
      let description = description.into();

      self.derive_accessible_description(move || ::std::rc::Rc::clone(&description))
    }

    /// Describes the control for a reader, kept current.
    pub fn derive_accessible_description<S: Into<::std::rc::Rc<str>>>(
      mut self,
      description: impl Fn() -> S + 'static,
    ) -> Self {
      self.$($path).+.access.description =
        Some(::std::rc::Rc::new(move || description().into()));

      self
    }

    /// Describes the control by the text of the node `description` captured, placed anywhere.
    pub fn set_described_by(mut self, description: &$crate::NodeHandle) -> Self {
      self.$($path).+.access.described_by = vec![*description];

      self
    }

    /// Clicks the control when `shortcut` (`Shortcut::primary('s')`: Control+S on Linux and
    /// Windows, Command+S on macOS) is pressed wherever focus is in its window, unless the
    /// focused control uses the key itself; a reader reads it as the control's key binding.
    pub const fn set_shortcut(mut self, shortcut: $crate::Shortcut) -> Self {
      self.$($path).+.access.shortcut = Some(shortcut);

      self
    }

    /// Runs `focus` when focus enters the control (`true`) or leaves it (`false`).
    pub fn handle_focus(mut self, focus: impl Fn(bool) + 'static) -> Self {
      self.$($path).+.focus = Some(::std::rc::Rc::new(focus));

      self
    }

    /// Runs `key` with each key while focus is on the control, after its behavior; it never
    /// decides what happens to the key.
    pub fn handle_key(mut self, key: impl Fn(&$crate::KeyEvent) + 'static) -> Self {
      self.$($path).+.key = Some(::std::rc::Rc::new(key));

      self
    }

    /// Writes the control's node into `handle` once built.
    pub const fn set_handle(mut self, handle: &$crate::NodeHandle) -> Self {
      self.$($path).+.handle = Some(*handle);

      self
    }
  };
}

pub(crate) use control_members;

/// A control: focus, events, decoration, a label, parts and accessibility once, over the
/// behavior that says what the interaction is ([`Behavior`]). A library's drawn control is
/// one.
///
/// Its node is a row (or a column, [`Control::set_axis`]) of its label, its parts and its
/// body, or the realization's own text view where its behavior's element is one. Every node
/// built inside it resolves the control's focus, press and states ([`State::follows_control`]):
/// a part styled `when(State::Checked, ..)` follows the control's check. A disabled control is
/// drawn at half opacity unless its style says otherwise, takes no focus and swallows its
/// input.
///
/// ```
/// use skene_gui::{behavior::Activate, prelude::*};
///
/// let presses = Signal::new(0);
///
/// Control::new(Activate::new().handle_activate(move || presses.update(|count| *count += 1)))
///   .set_padding(8)
///   .set_background(SystemColors::CONTROL.when(State::Hovered, SystemColors::CONTROL_HOVER))
///   .set_label("Press");
/// ```
#[must_use]
pub struct Control<B: Behavior> {
  behavior: B,
  axis: Axis,
  pub(crate) members: Members,
  parts: Vec<Child>,
  child: Option<Child>,
}

impl<B: Behavior> Control<B> {
  /// A control over `behavior`, laid out along a row, with no label, part or body.
  pub fn new(behavior: B) -> Self {
    Self {
      behavior,
      axis: Axis::Horizontal,
      members: Members::default(),
      parts: Vec::new(),
      child: None,
    }
  }

  /// A control over `behavior` carrying `members`: what a built-in that learns its behavior
  /// only as it is built (a radio, from its group) composes.
  pub(crate) fn with_members(behavior: B, members: Members) -> Self {
    Self {
      members,
      ..Self::new(behavior)
    }
  }

  /// Its behavior.
  pub const fn behavior(&self) -> &B {
    &self.behavior
  }

  /// This control with its behavior changed by `change`.
  pub fn map_behavior(mut self, change: impl FnOnce(B) -> B) -> Self {
    self.behavior = change(self.behavior);

    self
  }

  /// Lays the label, the parts and the body along `axis`; horizontal where unset.
  pub const fn set_axis(mut self, axis: Axis) -> Self {
    self.axis = axis;

    self
  }

  control_members!(members);

  /// Appends `view` as the control's `part`, after its leading label.
  pub fn add_part(mut self, part: Part, view: impl ChildOf<Self>) -> Self {
    self.parts.push(Box::new(move |cx: &mut Cx| {
      cx.part = Some(part);
      cx.presentational = part.is_presentational();

      view.build_child(cx)
    }));

    self
  }

  /// The body, after the parts; last.
  pub fn set_child(mut self, child: impl ChildOf<Self>) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl<B: Behavior> View for Control<B> {
  #[expect(clippy::too_many_lines, reason = "a control's every step, in order")]
  fn build(self, cx: &mut Cx) -> Node {
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
    let kind = match behavior.element() {
      Element::Box => NodeKind::Layout(match axis {
        Axis::Horizontal => LayoutKind::Row,
        Axis::Vertical => LayoutKind::Column,
      }),
      Element::Editable { lines } => NodeKind::Editable {
        multiline: lines > 1,
      },
    };
    let spec = NodeSpec::new(kind)
      .with_role(role)
      .with_interaction(Interaction::Control {
        drag: behavior.drag_axes(),
      });
    let beside = placement == LabelPlacement::Beside && members.label.is_some();
    let segment = beside.then(|| cx.create(NodeSpec::new(NodeKind::Segment)));
    let id = cx.create(spec);
    let mut record = Record::new();

    record.set(
      keys::OPACITY,
      Value::from(1.).when(State::Disabled, Value::from(0.5)),
    );
    record.fold(&members.record);
    finish(id, record, cx, members.handle.as_ref());

    let disabled = members.disabled.clone();
    let disabled = Memo::new(move || disabled.as_ref().is_some_and(|disabled| disabled()));
    let invalid = members.access.invalid.clone();
    let label_name = members.label.as_ref().and_then(|label| label.name.clone());
    let named_by_content = role.is_named_by_content();
    let handle = {
      let (access_name, label_name) = (members.access.name.clone(), label_name.clone());

      ControlHandle::new(
        id,
        OPS,
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

      tree::set_control_state::<Active>(id, move || {
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

      tree::add_access::<Active>(id, move || {
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

      tree::handle_control::<Active>(
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
      tree::handle_focus::<Active>(id, move |focused| focus(focused));
    }

    if let Some(key) = members.key {
      tree::handle_key::<Active, _>(id, move |event| {
        key(event);

        Bubbling::Continue
      });
    }

    behavior.attach(&handle);

    let mut inner = cx.child();

    inner.control = Some(id);

    let label = members.label.map(|label| -> Child {
      let view = label.view;

      Box::new(move |cx: &mut Cx| {
        cx.part = Some(Part::Label);
        cx.presentational = named_by_content;
        cx.answering = beside;

        let node = view(cx);

        caption.set(node.id());

        node
      })
    });
    let (mut leading, mut trailing, mut besides) = (None, None, None);

    match placement {
      LabelPlacement::Leading => leading = label,
      LabelPlacement::Trailing => trailing = label,
      LabelPlacement::Beside => besides = label,
    }

    let children: Vec<Child> = leading
      .into_iter()
      .chain(parts)
      .chain(child)
      .chain(trailing)
      .collect();

    attach_children(id, children, &inner);
    behavior.mounted(&handle);

    match (segment, besides) {
      (Some(segment), Some(label)) => {
        let label = label(&mut inner.child());

        tree::attach::<Active>(segment, label.id(), 0);
        tree::attach::<Active>(segment, id, 1);

        Node(segment)
      }
      _ => Node(id),
    }
  }
}

/// A button with no style of its own: a [`Control`] over [`behavior::Activate`].
pub type BasicButton = Control<behavior::Activate>;
/// A switch or a checkbox with no style of its own: a [`Control`] over [`behavior::Toggle`].
pub type BasicCheckbox = Control<behavior::Toggle>;
/// A slider with no style of its own: a [`Control`] over [`behavior::Slider`].
pub type BasicSlider = Control<behavior::Slider>;
/// A radio group with no style of its own: a [`Control`] over [`behavior::RadioGroup`].
pub type BasicRadioGroup = Control<behavior::RadioGroup>;
/// A radio with no style of its own: a [`Control`] over [`behavior::Radio`].
pub type BasicRadio = Control<behavior::Radio>;
/// A list box with no style of its own: a [`Control`] over [`behavior::Listbox`].
pub type BasicListbox = Control<behavior::Listbox>;
/// A list box's option with no style of its own: a [`Control`] over
/// [`behavior::ListboxOption`].
pub type BasicListboxOption = Control<behavior::ListboxOption>;
/// A text field with no style of its own: a [`Control`] over a one-line
/// [`behavior::TextEntry`].
pub type BasicTextField = Control<behavior::TextEntry>;
/// A text editor with no style of its own: a [`Control`] over a [`behavior::TextEntry`] of
/// several lines.
pub type BasicTextEditor = Control<behavior::TextEntry>;
