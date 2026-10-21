//! A picker: one choice of a list, shown closed, its options in a popup.

use std::rc::Rc;

use crate::{
  AxisAlignment, Border, Color, Dimension, Flexibility, Inset, State, SystemColors, Value,
  behavior::{Choice, Controlled, Listbox, One, Options as _, Popup},
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
    controls::{
      Pending,
      choice::{GroupOptions, HasChoice, HasOptions, OptionSegment},
      control_surfaces, focus_ring, marks,
    },
    wraps_itself,
  },
};

/// One option of a [`Picker`]'s list, as its item builder makes it: its caption, taking the
/// room its row leaves, then its mark ([`PickerItem::default_mark`]). Its list gives it its
/// behavior and its value as it builds it.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 before its text and 6 after its
/// mark, its corners rounded by 6, its text in [`SystemColors::INK`], filled with
/// [`SystemColors::CONTROL_HOVER`] while focus is on it (where the keys and the pointer move
/// it).
#[must_use]
pub struct PickerItem<R: Realization> {
  /// The control it composes, its defaults set, given its behavior by its list.
  control: crate::view::Control<Pending, R>,
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
  fn into_control<B: crate::behavior::Behavior>(self, behavior: B) -> crate::view::Control<B, R> {
    self
      .control
      .with_behavior(behavior)
      .add_part(Part::Mark, self.mark)
  }
}

/// What a [`PickerItem`] carries beside every control's members: its mark.
pub trait PickerItemBase<R: Realization>: Wraps<Inner = PickerItem<R>> {
  /// An option captioned by `label`: a text names it, a view by the texts it shows or through
  /// [`Label::set_name`]. The caption takes the room the row leaves.
  fn new(label: impl Into<Label<R>>) -> Self {
    let label = label.into().wrap_view(|view| {
      Flexible::new()
        .set_flexibility(Flexibility {
          grow: 1.,
          shrink: 1.,
          initial: Dimension::Auto,
        })
        .set_child(view)
    });
    let control = crate::view::Control::new(Pending)
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
      .set_label(label);

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

/// What makes a picker's option from its value and its caption.
type ItemBuilder<T, R> = Rc<dyn Fn(&T, Label<R>) -> PickerItem<R>>;

/// One choice of a list, shown closed, with its caption: Skene's picker, drawn on every
/// platform. It holds a `T`, each option carrying one; its face is a [`crate::view::Control`]
/// over a [`Popup`], and its options are real controls in a list box in an [`Overlay`] below
/// it, each from its item builder ([`PickerBase::set_item_builder`]).
///
/// Closed, `Space`, `Alt` with an arrow down and `F4` open it, the arrows choose without
/// opening, and typing finds an option by its name. Open, focus is on its options: the arrows
/// move it, `Enter`, `Space` and a click choose and close, `Escape` closes, `Tab` chooses and
/// moves on.
///
/// It is controlled: it shows what [`ChoiceBase::derive_chosen`] returns, asks
/// [`ChoiceBase::handle_choose`] for each choice, and shows the truth. A chosen value no option
/// carries shows nothing; a picker that starts with nothing chosen holds an `Option`, its
/// options tagged `Some(..)`.
///
/// Its caption sits before it among its siblings, a press on it focusing the picker. Its
/// face: 34 pixels tall at least, padded 7 above and below and 14 at the sides, its corners
/// rounded by 8, filled with [`SystemColors::CONTROL`] and its hover and pressed variants
/// (pressed while open), the chosen option's name taking the room the face leaves, as wide
/// as the widest option's, then its mark ([`Picker::default_mark`]); a ring of 2 in the
/// accent while focus shows. Its list: [`Picker::default_list`].
///
/// [`ChoiceBase::derive_chosen`]: crate::view::ChoiceBase::derive_chosen
/// [`ChoiceBase::handle_choose`]: crate::view::ChoiceBase::handle_choose
#[must_use]
pub struct Picker<T: Choice, R: Realization> {
  /// The face it composes, its defaults set.
  control: crate::view::Control<Popup<T>, R>,
  /// The list's behavior, which holds the choice.
  list: Listbox<One<T>>,
  /// Its options' runs.
  segments: Vec<OptionSegment<T, R>>,
  /// The face's mark.
  mark: AnyView<R>,
  /// The box around the open list.
  list_box: Container<R>,
  /// What makes each option.
  builder: ItemBuilder<T, R>,
}

wraps_itself!(Picker<T, R> where T: Choice, R: Realization);

control_surfaces!([T: Choice, R: Realization] Picker<T, R>, R => control);

impl<T: Choice, R: Realization> HasChoice<T> for Picker<T, R> {
  fn change_chosen(&mut self, change: impl FnOnce(&mut Controlled<T>)) {
    self.list.roster().change_chosen(change);
  }
}

impl<T: Choice, R: Realization> HasOptions<T, R> for Picker<T, R> {
  fn segments_mut(&mut self) -> &mut Vec<OptionSegment<T, R>> {
    &mut self.segments
  }
}

impl<T: Choice, R: Realization> Picker<T, R> {
  /// The default mark: [`marks::chevron`], 12 by 8 pixels.
  pub fn default_mark() -> Painted<R> {
    Painted::new(marks::chevron).set_width(12).set_height(8)
  }

  /// The default list: [`SystemColors::FIELD`] inside a border of 1 in
  /// [`SystemColors::BORDER`], its corners rounded by 8, padded 4, 320 pixels tall at most
  /// (about ten options), its options scrolling beyond.
  pub fn default_list() -> Container<R> {
    Container::new()
      .set_background(SystemColors::FIELD)
      .set_border(Border::new(1))
      .set_border_color(SystemColors::BORDER)
      .set_corner_radius(8)
      .set_padding(Inset::all(4))
      .set_max_height(320)
  }
}

/// What a [`Picker`] carries beside every control's members, its choice and its options: its
/// mark, its list and what makes each option.
pub trait PickerBase<T: Choice, R: Realization>: Wraps<Inner = Picker<T, R>> {
  /// A picker labelled `label`, with no options.
  fn new(label: impl Into<Label<R>>) -> Self {
    let list = Listbox::<One<T>>::new();
    let control = crate::view::Control::new(Popup::new(list.roster()))
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
      segments: Vec::new(),
      mark: Picker::<T, R>::default_mark().into_any(),
      list_box: Picker::<T, R>::default_list(),
      builder: Rc::new(|_, label| PickerItem::new(label)),
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

  /// Makes each option with `builder`, from its value and its caption: a styled
  /// [`PickerItem`]; `PickerItem::new(label)` where unset.
  fn set_item_builder<I: Wraps<Inner = PickerItem<R>>>(
    mut self,
    builder: impl Fn(&T, Label<R>) -> I + 'static,
  ) -> Self {
    self.inner_mut().builder = Rc::new(move |value, label| builder(value, label).into_inner());

    self
  }
}

impl<T: Choice, R: Realization> PickerBase<T, R> for Picker<T, R> {}

impl<T: Choice, R: Realization> AnyKind for Picker<T, R> {}

impl<T: Choice, R: Realization> View<R> for Picker<T, R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      control,
      list,
      segments,
      mark,
      list_box,
      builder,
    } = self;
    let roster = list.roster();
    let options = GroupOptions::new(segments);

    roster.set_options(Rc::new(options.clone()));

    let open = control.behavior().open();
    let shown = {
      let (options, roster) = (options.clone(), Rc::clone(&roster));

      move || {
        roster
          .chosen()
          .and_then(|chosen| options.name_of(&chosen))
          .map(|name| name.to_string())
          .unwrap_or_default()
      }
    };
    let names = {
      let options = options.clone();

      move || {
        (0..options.count())
          .filter_map(|index| options.name(index))
          .collect::<Vec<Rc<str>>>()
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
      let values = options.clone();

      Column::<R>::new().derive_children(
        move || values.values(),
        Clone::clone,
        move |value: T| {
          let member = Listbox::with_roster(Rc::clone(&roster)).member(value.clone());

          builder(&value, options.caption(&value)).into_control(member)
        },
      )
    };
    let listbox = crate::view::Control::<Listbox<One<T>>, R>::new(list)
      .set_axis(Axis::Vertical)
      .set_child(items);
    let overlay =
      Overlay::<R>::popup()
        .bind_open(open)
        .set_child(AsPart::<R>::new(Part::List).set_child(
          list_box.set_child(Scroll::<R>::new(ScrollAxes::Vertical).set_child(listbox)),
        ));

    control
      .add_part(Part::Value, text)
      .add_part(Part::Mark, mark)
      .set_child(overlay)
      .build(cx)
  }
}
