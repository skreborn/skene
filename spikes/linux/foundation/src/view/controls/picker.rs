//! A picker: one choice of a list, shown closed, its options in a popup.

use std::rc::Rc;

use crate::{
  AxisAlignment, Border, Color, Dimension, Flexibility, Inset, Signal, State, SystemColors, Value,
  behavior::{self, Listbox, Popup},
  layout::Axis,
  seam::{
    Realization,
    control::{LabelPlacement, Part},
  },
  view::{
    AnyKind, AnyView, AsPart, AsPartBase, Column, ColumnBase, Container, ContainerBase,
    ControlBase, ControlMembersBase, Cx, DecorationBase, Flexible, FlexibleBase, Label, Node,
    Overlay, OverlayBase, Painted, PaintedBase, RunBase, Scroll, ScrollAxes, ScrollBase, Text,
    TextBase, View, Wraps,
    controls::{control_surfaces, focus_ring, marks},
    wraps_itself,
  },
};

/// One option of a [`Picker`]'s list, as its item builder makes it: its text, taking the room
/// its row leaves, then its mark ([`PickerItem::default_mark`]). Its list gives it its behavior
/// ([`crate::behavior::ListboxOption`]) as it builds it; its text is its name.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 before its text and 6 after its
/// mark, its corners rounded by 6, its text in [`SystemColors::INK`], filled with
/// [`SystemColors::CONTROL_HOVER`] while focus is on it (where the keys and the pointer move
/// it).
#[must_use]
pub struct PickerItem<R: Realization> {
  /// The control it composes, its defaults set, over a behavior its list replaces.
  control: crate::view::Control<behavior::ListboxOption, R>,
  /// The mark shown while it is chosen.
  mark: AnyView<R>,
}

wraps_itself!(PickerItem<R> where R: Realization);

control_surfaces!([R: Realization] PickerItem<R>, R => control);

impl<R: Realization> PickerItem<R> {
  /// The default mark: [`marks::check_mark`], 14 by 12 pixels, in the accent.
  pub fn default_mark() -> Painted<R> {
    Painted::new(marks::check_mark)
      .set_width(14)
      .set_height(12)
      .set_color(SystemColors::ACCENT)
  }

  /// The control this option is, over `behavior`.
  fn into_control(
    self,
    behavior: behavior::ListboxOption,
  ) -> crate::view::Control<behavior::ListboxOption, R> {
    self
      .control
      .map_behavior(|_| behavior)
      .add_part(Part::Mark, self.mark)
  }
}

/// What a [`PickerItem`] carries beside every control's members: its mark.
pub trait PickerItemBase<R: Realization>: Wraps<Inner = PickerItem<R>> {
  /// An option labelled `label`.
  fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();
    let text = Flexible::new()
      .set_flexibility(Flexibility {
        grow: 1.,
        shrink: 1.,
        initial: Dimension::Auto,
      })
      .set_child(Text::<R>::new(label.clone()).set_max_lines(1));
    let control = crate::view::Control::new(Listbox::new().member(0))
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
      .set_label(Label::new(text).set_name(label));

    Self::wrap(PickerItem {
      control,
      mark: PickerItem::<R>::default_mark().into_any(),
    })
  }

  /// The mark, in place of [`PickerItem::default_mark`]: any view, drawn under the option's
  /// states (`State::Checked` while chosen).
  fn set_mark(mut self, mark: impl View<R>) -> Self {
    self.inner_mut().mark = mark.into_any();

    self
  }
}

impl<R: Realization> PickerItemBase<R> for PickerItem<R> {}

/// A picker's face with no style of its own: a [`crate::view::Control`] over a [`Popup`],
/// composed by its user with an [`Overlay`] holding a [`crate::view::BasicListbox`] of
/// [`crate::view::BasicListboxOption`]s over the same roster.
pub type BasicPicker<R> = crate::view::Control<Popup, R>;

/// What makes a picker's option from its index and its text.
type ItemBuilder<R> = Rc<dyn Fn(usize, &str) -> PickerItem<R>>;

/// One choice of a list, shown closed, with its caption: Skene's picker, drawn on every
/// platform. Its face is a [`crate::view::Control`] over a [`Popup`]; its options are real
/// controls in a list box in an [`Overlay`] below it, each from its item builder
/// ([`PickerBase::set_item_builder`]).
///
/// Closed, `Space`, `Alt` with an arrow down and `F4` open it, the arrows choose without
/// opening, and typing finds an option by its name. Open, focus is on its options: the arrows
/// move it, `Enter`, `Space` and a click choose and close, `Escape` closes, `Tab` chooses and
/// moves on.
///
/// It is controlled: it shows what [`PickerBase::derive_chosen`] returns, asks
/// [`PickerBase::handle_choose`] for each choice, and shows the truth.
///
/// Its caption sits before it among its siblings, a press on it focusing the picker. Its
/// face: 34 pixels tall at least, padded 7 above and below and 14 at the sides, its corners
/// rounded by 8, filled with [`SystemColors::CONTROL`] and its hover and pressed variants
/// (pressed while open), the chosen option's text taking the room the face leaves, as wide
/// as the widest option's, then its mark ([`Picker::default_mark`]); a ring of 2 in the
/// accent while focus shows. Its list: [`Picker::default_list`].
#[must_use]
pub struct Picker<R: Realization> {
  /// The face it composes, its defaults set, over a behavior replaced as it is built.
  control: crate::view::Control<Popup, R>,
  /// The list's behavior, which holds the choice.
  list: Listbox,
  /// The options' texts.
  options: Rc<dyn Fn() -> Vec<String>>,
  /// The face's mark.
  mark: AnyView<R>,
  /// The box around the open list.
  list_box: Container<R>,
  /// What makes each option.
  builder: ItemBuilder<R>,
}

wraps_itself!(Picker<R> where R: Realization);

control_surfaces!([R: Realization] Picker<R>, R => control);

impl<R: Realization> Picker<R> {
  /// The default mark: [`marks::chevron`], 12 by 8 pixels.
  pub fn default_mark() -> Painted<R> {
    Painted::new(marks::chevron).set_width(12).set_height(8)
  }

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
}

/// What a [`Picker`] carries beside every control's members: its choice, its options, its
/// mark, its list and what makes each option.
pub trait PickerBase<R: Realization>: Wraps<Inner = Picker<R>> {
  /// A picker labelled `label`, with no options.
  fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();
    let list = Listbox::new();
    let control = crate::view::Control::new(Popup::new(list.roster(), Vec::new))
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
      .set_label(label);

    Self::wrap(Picker {
      control,
      list,
      options: Rc::new(Vec::new),
      mark: Picker::<R>::default_mark().into_any(),
      list_box: Picker::<R>::default_list(),
      builder: Rc::new(|_, name| PickerItem::new(name)),
    })
  }

  /// The chosen option's index, or none.
  fn set_chosen(self, chosen: Option<usize>) -> Self {
    self.map_inner(|mut picker| {
      picker.list = picker.list.set_chosen(chosen);

      picker
    })
  }

  /// The chosen option's index, or none, kept current.
  fn derive_chosen(self, chosen: impl Fn() -> Option<usize> + 'static) -> Self {
    self.map_inner(|mut picker| {
      picker.list = picker.list.derive_chosen(chosen);

      picker
    })
  }

  /// Runs `choose` with the option the reader chose.
  fn handle_choose(self, choose: impl Fn(Option<usize>) + 'static) -> Self {
    self.map_inner(|mut picker| {
      picker.list = picker.list.handle_choose(choose);

      picker
    })
  }

  /// Shows `chosen` and writes each choice into it.
  fn bind_chosen(self, chosen: Signal<Option<usize>>) -> Self {
    self.map_inner(|mut picker| {
      picker.list = picker.list.bind_chosen(chosen);

      picker
    })
  }

  /// The face's mark, in place of [`Picker::default_mark`]: any view, drawn under the face's
  /// states (`State::Open` while open).
  fn set_mark(mut self, mark: impl View<R>) -> Self {
    self.inner_mut().mark = mark.into_any();

    self
  }

  /// The box around the open list, in place of [`Picker::default_list`]; the options are
  /// placed in it, so a child it holds is replaced.
  fn set_list(mut self, list: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().list_box = list.into_inner();

    self
  }

  /// Makes each option with `builder`, from its index and its text: a styled [`PickerItem`];
  /// `PickerItem::new(text)` where unset.
  fn set_item_builder<I: Wraps<Inner = PickerItem<R>>>(
    mut self,
    builder: impl Fn(usize, &str) -> I + 'static,
  ) -> Self {
    self.inner_mut().builder = Rc::new(move |index, name| builder(index, name).into_inner());

    self
  }

  /// The options' texts.
  fn set_options<S: Into<String>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Vec<String> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.clone())
  }

  /// The options' texts, kept current.
  fn derive_options<S: Into<String>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.inner_mut().options = Rc::new(move || options().into_iter().map(Into::into).collect());

    self
  }
}

impl<R: Realization> PickerBase<R> for Picker<R> {}

impl<R: Realization> AnyKind for Picker<R> {}

impl<R: Realization> View<R> for Picker<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
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
        Text::<R>::derive(shown)
          .set_max_lines(1)
          .derive_reserved_texts(names),
      );
    let items = {
      let options = Rc::clone(&options);

      Column::<R>::new().derive_children(
        move || options().into_iter().enumerate().collect(),
        Clone::clone,
        move |(index, name): (usize, String)| {
          let member = Listbox::with_roster(Rc::clone(&roster)).member(index);

          builder(index, &name).into_control(member)
        },
      )
    };
    let listbox = crate::view::Control::<Listbox, R>::new(list)
      .set_axis(Axis::Vertical)
      .set_child(items);
    let overlay = Overlay::<R>::popup().bind_open(open).set_child(
      AsPart::<R>::new(Part::List).set_child(
        list_box.set_child(
          Scroll::<R>::new(ScrollAxes::Vertical)
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
