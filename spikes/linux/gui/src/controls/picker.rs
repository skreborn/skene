//! A picker: one choice of a list, shown closed, its options in a popup.

use std::rc::Rc;

use skene_foundation::{
  AxisAlignment, Border, Color, Dimension, Flexibility, Inset, State, SystemColors, Value,
  behavior::{self, Listbox, Popup},
  layout::Axis,
  seam::control::{LabelPlacement, Part},
};

use crate::{
  AsPart, Column, Container, Control, Flexible, Label, Overlay, Painted, Scroll, ScrollAxes,
  Signal, Text,
  control::control_members,
  controls::{focus_ring, marks},
  view::{AnyView, Cx, Node, View},
};

/// One option of a [`Picker`]'s list, as its item builder makes it: its text, taking the room
/// its row leaves, then its mark ([`PickerItem::default_mark`]). Its list gives it its behavior
/// ([`skene_foundation::behavior::ListboxOption`]) as it builds it; its text is its name.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 before its text and 6 after its
/// mark, its corners rounded by 6, its text in [`SystemColors::INK`], filled with
/// [`SystemColors::CONTROL_HOVER`] while focus is on it (where the keys and the pointer move
/// it).
#[must_use]
pub struct PickerItem {
  /// The control it composes, its defaults set, over a behavior its list replaces.
  pub(super) control: Control<behavior::ListboxOption>,
  /// The mark shown while it is chosen.
  mark: AnyView,
}

impl PickerItem {
  /// An option labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();
    let text = Flexible::new()
      .set_flexibility(Flexibility {
        grow: 1.,
        shrink: 1.,
        initial: Dimension::Auto,
      })
      .set_child(Text::new(label.clone()).set_max_lines(1));

    Self {
      control: Control::new(Listbox::new().member(0)),
      mark: Self::default_mark().into_any(),
    }
    .set_min_height(30)
    .set_padding(Inset::new(0, 6, 0, 12))
    .set_corner_radius(6)
    .set_item_alignment(AxisAlignment::Center)
    .set_spacing(8)
    .set_color(SystemColors::INK)
    .set_background(
      Value::from(Color::TRANSPARENT).when(State::Focused, SystemColors::CONTROL_HOVER),
    )
    .set_label_placement(LabelPlacement::Leading)
    .set_label(Label::new(text).set_name(label))
  }

  /// The default mark: [`marks::check_mark`], 14 by 12 pixels, in the accent.
  pub fn default_mark() -> Painted {
    Painted::new(marks::check_mark)
      .set_width(14)
      .set_height(12)
      .set_color(SystemColors::ACCENT)
  }

  /// The mark, in place of [`PickerItem::default_mark`]: any view, drawn under the option's
  /// states (`State::Checked` while chosen).
  pub fn set_mark(mut self, mark: impl View) -> Self {
    self.mark = mark.into_any();

    self
  }

  control_members!(control.members);

  /// The control this option is, over `behavior`.
  fn into_control(self, behavior: behavior::ListboxOption) -> Control<behavior::ListboxOption> {
    self
      .control
      .map_behavior(|_| behavior)
      .add_part(Part::Mark, self.mark)
  }
}

/// A picker's face with no style of its own: a [`Control`] over a [`Popup`], composed by its
/// user with an [`Overlay`] holding a [`crate::BasicListbox`] of
/// [`crate::BasicListboxOption`]s over the same roster.
pub type BasicPicker = Control<Popup>;

/// What makes a picker's option from its index and its text.
type ItemBuilder = Rc<dyn Fn(usize, &str) -> PickerItem>;

/// One choice of a list, shown closed, with its caption: Skene's picker, drawn on every
/// platform. Its face is a [`Control`] over a [`Popup`]; its options are real controls in a
/// list box in an [`Overlay`] below it, each from its item builder
/// ([`Picker::set_item_builder`]).
///
/// Closed, `Space`, `Alt` with an arrow down and `F4` open it, the arrows choose without
/// opening, and typing finds an option by its name. Open, focus is on its options: the arrows
/// move it, `Enter`, `Space` and a click choose and close, `Escape` closes, `Tab` chooses and
/// moves on.
///
/// It is controlled: it shows what [`Picker::derive_chosen`] returns, asks
/// [`Picker::handle_choose`] for each choice, and shows the truth.
///
/// Its caption sits before it among its siblings, a press on it focusing the picker. Its
/// face: 34 pixels tall at least, padded 7 above and below and 14 at the sides, its corners
/// rounded by 8, filled with [`SystemColors::CONTROL`] and its hover and pressed variants
/// (pressed while open), the chosen option's text taking the room the face leaves, as wide
/// as the widest option's, then its mark ([`Picker::default_mark`]); a ring of 2 in the
/// accent while focus shows. Its list: [`Picker::default_list`].
#[must_use]
pub struct Picker {
  /// The face it composes, its defaults set, over a behavior replaced as it is built.
  pub(super) control: Control<Popup>,
  /// The list's behavior, which holds the choice.
  list: Listbox,
  /// The options' texts.
  options: Rc<dyn Fn() -> Vec<String>>,
  /// The face's mark.
  mark: AnyView,
  /// The box around the open list.
  pub(super) list_box: Container,
  /// What makes each option.
  builder: ItemBuilder,
}

impl Picker {
  /// A picker labelled `label`, with no options.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();
    let list = Listbox::new();

    Self {
      control: Control::new(Popup::new(list.roster(), Vec::new)),
      list,
      options: Rc::new(Vec::new),
      mark: Self::default_mark().into_any(),
      list_box: Self::default_list(),
      builder: Rc::new(|_, name| PickerItem::new(name)),
    }
    .set_min_height(34)
    .set_padding(Inset::new(7, 14, 7, 14))
    .set_corner_radius(8)
    .set_item_alignment(AxisAlignment::Center)
    .set_spacing(8)
    .set_color(SystemColors::INK)
    .set_background(
      Value::from(SystemColors::CONTROL)
        .when(State::Hovered, SystemColors::CONTROL_HOVER)
        .when(State::Pressed, SystemColors::CONTROL_PRESSED)
        .when(State::Open, SystemColors::CONTROL_PRESSED)
        .when(State::Disabled, SystemColors::CONTROL),
    )
    .set_ring(focus_ring())
    .set_ring_color(SystemColors::ACCENT)
    .set_label(label)
  }

  /// The default mark: [`marks::chevron`], 12 by 8 pixels.
  pub fn default_mark() -> Painted {
    Painted::new(marks::chevron).set_width(12).set_height(8)
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

  /// The options' texts.
  pub fn set_options<S: Into<String>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Vec<String> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.clone())
  }

  /// The options' texts, kept current.
  pub fn derive_options<S: Into<String>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.options = Rc::new(move || options().into_iter().map(Into::into).collect());

    self
  }

  /// The chosen option's index, or none.
  pub fn set_chosen(mut self, chosen: Option<usize>) -> Self {
    self.list = self.list.set_chosen(chosen);

    self
  }

  /// The chosen option's index, or none, kept current.
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<usize> + 'static) -> Self {
    self.list = self.list.derive_chosen(chosen);

    self
  }

  /// Runs `choose` with the option the reader chose.
  pub fn handle_choose(mut self, choose: impl Fn(Option<usize>) + 'static) -> Self {
    self.list = self.list.handle_choose(choose);

    self
  }

  /// Shows `chosen` and writes each choice into it.
  pub fn bind_chosen(mut self, chosen: Signal<Option<usize>>) -> Self {
    self.list = self.list.bind_chosen(chosen);

    self
  }

  /// The face's mark, in place of [`Picker::default_mark`]: any view, drawn under the face's
  /// states (`State::Open` while open).
  pub fn set_mark(mut self, mark: impl View) -> Self {
    self.mark = mark.into_any();

    self
  }

  /// The box around the open list, in place of [`Picker::default_list`]; the options are
  /// placed in it, so a child it holds is replaced.
  pub fn set_list(mut self, list: Container) -> Self {
    self.list_box = list;

    self
  }

  /// Makes each option with `builder`, from its index and its text: a styled [`PickerItem`];
  /// `PickerItem::new(text)` where unset.
  pub fn set_item_builder(mut self, builder: impl Fn(usize, &str) -> PickerItem + 'static) -> Self {
    self.builder = Rc::new(builder);

    self
  }

  control_members!(control.members);
}

impl View for Picker {
  fn build(self, cx: &mut Cx) -> Node {
    let Self {
      control,
      list,
      options,
      mark,
      list_box,
      builder,
    } = self;
    let roster = list.roster();
    let names = {
      let options = Rc::clone(&options);

      move || {
        options()
          .into_iter()
          .map(Rc::from)
          .collect::<Vec<Rc<str>>>()
      }
    };
    let popup = Popup::new(Rc::clone(&roster), names.clone());
    let open = popup.open();
    let shown = {
      let (options, roster) = (Rc::clone(&options), Rc::clone(&roster));

      move || {
        roster
          .chosen()
          .and_then(|index| options().into_iter().nth(index))
          .unwrap_or_default()
      }
    };
    let text = Flexible::new()
      .set_flexibility(Flexibility {
        grow: 1.,
        shrink: 1.,
        initial: Dimension::Auto,
      })
      .set_child(
        Text::derive(shown)
          .set_max_lines(1)
          .derive_reserved_texts(names),
      );
    let items = {
      let options = Rc::clone(&options);

      Column::new().derive_children(
        move || options().into_iter().enumerate().collect(),
        Clone::clone,
        move |(index, name): (usize, String)| {
          let member = Listbox::with_roster(Rc::clone(&roster)).member(index);

          builder(index, &name).into_control(member)
        },
      )
    };
    let listbox = Control::new(list).set_axis(Axis::Vertical).set_child(items);
    let overlay = Overlay::popup().bind_open(open).set_child(
      AsPart::new(Part::List).set_child(
        list_box.set_child(
          Scroll::new(ScrollAxes::Vertical)
            .set_max_height(310)
            .set_child(listbox),
        ),
      ),
    );

    control
      .map_behavior(|_| popup)
      .add_part(Part::Value, text)
      .add_part(Part::Mark, mark)
      .set_child(overlay)
      .build(cx)
  }
}
