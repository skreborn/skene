//! A ready-made dropdown choice and its options, in the Simple design language.

use skene_foundation::{
  AnyChosen,
  Arrangement,
  Dimension,
  FlexBase,
  HasChild,
  HasChildren,
  ItemAlignment,
  ListboxContext,
  Spacing,
};
use skene_gui::{
  BasicPickerBase,
  Choice,
  Color,
  Container,
  ContainerBase,
  ControlBase,
  DecorationBase,
  DecorationWithCornerRadius,
  Inset,
  Label,
  ListboxOption,
  Part,
  Picker,
  Row,
  RowBase,
  RunBase,
  State,
  Text,
  TextBase,
  When,
  reactive::{Get, Memo, use_context},
};

use crate::{SimplePalette, as_part, shape};

/// A ready-made dropdown choice, a preset of [`Picker`]: a face in the field chrome and
/// a list on the card surface.
///
/// The face's hairline becomes the ring while focused or open, and it shows the chosen
/// option's name and the indicator `▾`. [`new`](Self::new) returns the picker, so every
/// member of a picker applies: its options, [`SimpleListboxOption`]s or any
/// [`ListboxOption`], tagged with values of the app's own type `T`, come from its
/// `set_options`, and the chosen value flows both ways. [`styled`](Self::styled)
/// dresses a picker built otherwise from the pieces [`face`](Self::face) and
/// [`list`](Self::list). The indicator is the face's
/// [`Part::Face`](skene_gui::Part::Face) and the list surface the picker's
/// [`Part::List`](skene_gui::Part::List), so both follow its state. Its colors are
/// [`SimplePalette`]'s tokens, inked in the palette's default unless a
/// [`Theme`](skene_gui::Theme) of the palette above redefines them.
///
/// ```
/// use skene_gui::prelude::*;
/// use skene_simple_gui::prelude::*;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
///   Large,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let picker = SimplePicker::new()
///   .set_label("Size")
///   .bind_chosen(size)
///   .set_options((
///     SimpleListboxOption::new(Size::Small, "Small"),
///     SimpleListboxOption::new(Size::Medium, "Medium"),
///     SimpleListboxOption::new(Size::Large, "Large"),
///   ));
/// ```
#[derive(Debug)]
pub enum SimplePicker {}

impl SimplePicker {
  /// A picker with no options and nothing chosen.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new<T: Choice>() -> Picker<T> {
    Self::styled(Picker::new())
  }

  /// `picker` in the field chrome, its face the chosen option's name and the indicator,
  /// its list on the card surface.
  pub fn styled<T: Choice>(picker: Picker<T>) -> Picker<T> {
    let shape = shape();

    picker
      .set_width(Dimension::Fraction(1.))
      .set_padding(shape.control_inset)
      .set_background(SimplePalette::CARD.when(State::Disabled, SimplePalette::DISABLED_SURFACE))
      .set_border_width(Inset::all(shape.field_ring))
      .set_border_color(
        SimplePalette::BORDER
          .when(State::FocusVisible, SimplePalette::RING)
          .when(State::Open, SimplePalette::RING),
      )
      .set_corner_radius(shape.control_radius)
      .set_list(Self::list())
      .set_face(|chosen: Memo<Option<T>>| {
        let options = use_context::<ListboxContext>();

        Self::face(move || {
          options
            .as_ref()
            .and_then(|options| options.roster().name_of(&AnyChosen::new(chosen.get())))
            .unwrap_or_default()
        })
      })
  }

  /// The face: the name `name` reads, then the indicator at the end, its
  /// [`Part::Face`](skene_gui::Part::Face), muted until the list opens.
  pub fn face(name: impl Fn() -> String + 'static) -> Row {
    let indicator = Text::new("\u{25be}")
      .set_color(SimplePalette::MUTED_FOREGROUND.when(State::Open, SimplePalette::FOREGROUND));

    Row::new()
      .set_spacing(Spacing::all(shape().indicator_spacing))
      .set_arrangement(Arrangement::SpaceBetween)
      .set_item_alignment(ItemAlignment::Center)
      .set_children((
        Text::derive(name)
          .set_color(SimplePalette::FOREGROUND.when(State::Disabled, SimplePalette::DISABLED_INK)),
        as_part(Part::Face, indicator),
      ))
  }

  /// The surface the open list draws its options on: the card surface inside a hairline
  /// border, rounded.
  pub fn list() -> Container {
    let shape = shape();

    Container::new()
      .set_padding(shape.list_inset)
      .set_background(SimplePalette::CARD)
      .set_border_width(Inset::all(1))
      .set_border_color(SimplePalette::BORDER)
      .set_corner_radius(shape.control_radius)
  }
}

/// A ready-made option of a picker or a listbox, a preset of [`ListboxOption`]:
/// highlighted while focused and filled in the primary look while chosen, its check
/// showing then, beside its caption.
///
/// [`new`](Self::new) returns the option, so every member of an option applies, its
/// caption naming it for the face and for type-ahead; [`styled`](Self::styled) dresses an
/// option built otherwise from the pieces [`check`](Self::check), its
/// [`Part::Mark`](skene_gui::Part::Mark), and [`label`](Self::label). Its colors are
/// [`SimplePalette`]'s tokens.
///
/// ```
/// use skene_gui::prelude::*;
/// use skene_simple_gui::prelude::*;
///
/// let apple = SimpleListboxOption::new(1, "Apple");
/// ```
#[derive(Debug)]
pub enum SimpleListboxOption {}

impl SimpleListboxOption {
  /// An option tagged with `value`, what its list is asked to choose when it is chosen,
  /// captioned and named `label`.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(value: impl Choice, label: impl Into<String>) -> ListboxOption {
    Self::styled(ListboxOption::new(value)).set_label(Self::label(label))
  }

  /// `option` padded and rounded like a row with no border, highlighted while focused and
  /// filled in the primary look while chosen, showing the Simple check.
  pub fn styled(option: ListboxOption) -> ListboxOption {
    let shape = shape();

    option
      .set_padding(shape.row_inset)
      .set_background(
        Color::TRANSPARENT
          .when(State::Focused, SimplePalette::SECONDARY_SURFACE)
          .when(State::Checked, SimplePalette::PRIMARY_SURFACE),
      )
      .set_border_width(Inset::all(0))
      .set_corner_radius(shape.mark_radius)
      .set_part(Part::Mark, Self::check())
  }

  /// The check: shown while the option is chosen, in the primary ink.
  pub fn check() -> Container {
    Container::new()
      .set_hidden(true.when(State::Checked, false))
      .set_child(Text::new("\u{2713}").set_color(SimplePalette::PRIMARY_INK))
  }

  /// The caption `label`, naming the option: in the foreground, the primary ink while the
  /// option is chosen.
  pub fn label(label: impl Into<String>) -> Label {
    let label = label.into();
    let caption = Text::new(label.as_str())
      .set_color(SimplePalette::FOREGROUND.when(State::Checked, SimplePalette::PRIMARY_INK));

    Label::new(caption).set_name(label)
  }
}
