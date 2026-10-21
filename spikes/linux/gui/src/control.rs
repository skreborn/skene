//! A control: focus, events, decoration, a label, parts and accessibility once, over the
//! behavior that says what its interaction is. Every built-in control is one, styled
//! (`Button` composes a `Control<Activate>`), and a library's drawn control is one too.

use skene_foundation::{
  behavior::{self, Behavior},
  view::{
    self as base, AsPartBase, ControlBase, ControlMembersBase, DecorationBase, MotionBase, RunBase,
    RunWithEntries,
  },
  wraps,
};

use crate::Active;

/// A control: focus, events, decoration, a label, parts and accessibility once, over the
/// behavior that says what the interaction is ([`Behavior`]). A library's drawn control is
/// one.
///
/// Its node is a row (or a column, `set_axis`) of its label, its parts and its body, or the
/// realization's own text view where its behavior's element is one. Every node built inside
/// it resolves every state but `FocusWithin` against the control (its hover, focus and press,
/// its caption's beside it, and its own states): a part styled `when(State::Checked, ..)`
/// follows the control's check, and one styled `when(State::Hovered, ..)` highlights while
/// the pointer is over the control or its caption. A box that acts inside it keeps its own
/// hover, focus and press. A disabled control, or one inside a disabled control, is drawn at
/// half opacity unless its style says otherwise, takes no focus and swallows its input.
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
pub struct Control<B: Behavior>(base::Control<B, Active>);

wraps!(thin [B: Behavior] Control<B> => base::Control<B, Active>, base::ControlCore<Active>, Active);

impl<B: Behavior> ControlBase<B, Active> for Control<B> {}

impl<B: Behavior> ControlMembersBase<Active> for Control<B> {}

impl<B: Behavior> DecorationBase for Control<B> {}

impl<B: Behavior> RunBase for Control<B> {}

impl<B: Behavior> RunWithEntries for Control<B> {}

impl<B: Behavior> MotionBase for Control<B> {}

/// Records its one child as a part of the control it is built inside: the child's states are
/// the control's, and a reader skips a part that is decoration. It realizes nothing of its
/// own; without a child it shows nothing.
#[must_use]
pub struct AsPart(base::AsPart<Active>);

wraps!([] AsPart => base::AsPart<Active>, Active);

impl AsPartBase<Active> for AsPart {}

/// A button with no style of its own: a [`Control`] over [`behavior::Activate`].
pub type BasicButton = Control<behavior::Activate>;
/// A switch or a checkbox with no style of its own: a [`Control`] over [`behavior::Toggle`].
pub type BasicCheckbox = Control<behavior::Toggle>;
/// A slider with no style of its own: a [`Control`] over [`behavior::Slider`].
pub type BasicSlider = Control<behavior::Slider>;
/// A radio group holding a `T`, with no style of its own: a [`Control`] over
/// [`behavior::RadioGroup`].
pub type BasicRadioGroup<T> = Control<behavior::RadioGroup<T>>;
/// A radio carrying a `T`, with no style of its own: a [`Control`] over [`behavior::Radio`].
pub type BasicRadio<T> = Control<behavior::Radio<T>>;
/// A list box choosing `C`, with no style of its own: a [`Control`] over
/// [`behavior::Listbox`].
pub type BasicListbox<C> = Control<behavior::Listbox<C>>;
/// A list box's option, with no style of its own: a [`Control`] over
/// [`behavior::ListboxOption`].
pub type BasicListboxOption<C> = Control<behavior::ListboxOption<C>>;
/// A text field with no style of its own: a [`Control`] over a one-line
/// [`behavior::TextEntry`].
pub type BasicTextField = Control<behavior::TextEntry>;
/// A text editor with no style of its own: a [`Control`] over a [`behavior::TextEntry`] of
/// several lines.
pub type BasicTextEditor = Control<behavior::TextEntry>;
/// A picker's face holding a `T`, with no style of its own: a [`Control`] over a
/// [`behavior::Popup`], composed by its user with an `Overlay` holding a [`BasicListbox`] of
/// [`BasicListboxOption`]s over the same roster.
pub type BasicPicker<T> = Control<behavior::Popup<T>>;
