//! A list box: options in a list that builds only those in view, one or several of them
//! chosen, whether built or not.

use std::{marker::PhantomData, rc::Rc};

use crate::{
  AxisAlignment, Border, Color, Effect, Inset, Spacing, State, SystemColors, Value,
  behavior::{
    AnyChoice, AnyChosen, Behavior, Chosen, Controlled, Listbox as ListboxBehavior, Options,
  },
  layout::Axis,
  lazy::ItemExtent,
  seam::{
    Realization,
    control::{LabelPlacement, Part},
  },
  untrack,
  view::{
    AnyKind, AsPart, AsPartBase, Container, ContainerBase, ControlBase, ControlMembersBase, Cx,
    DecorationBase, ItemHeightBase, Label, Lazy, LazyBase, LazyHandle, Node, RunBase, Thin, View,
    Wraps,
    controls::{
      Pending,
      choice::{GroupOptions, HasChoice, HasOptions, OptionSegment},
      control_surfaces, focus_ring,
    },
    wraps_itself,
  },
};

/// One option of a [`Listbox`], as its item builder makes it: its caption. Its list box gives
/// it its behavior and its value as it builds it.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 at its sides, its corners rounded by
/// 6, its text in [`SystemColors::INK`], filled with [`SystemColors::CONTROL_HOVER`] while
/// focus is on it, and with the accent, its text in [`SystemColors::ON_ACCENT`], while chosen;
/// a ring of 2 in the accent while focus shows.
#[must_use]
pub struct ListboxItem<R: Realization> {
  /// The control it composes, captioned, its defaults set and its behavior given as its list
  /// box builds it.
  control: crate::view::Control<Pending, R>,
}

wraps_itself!(ListboxItem<R> where R: Realization);

control_surfaces!([R: Realization] ListboxItem<R>, R => control);

impl<R: Realization> ListboxItem<R> {
  /// The control this option is, over `behavior`, its defaults set.
  fn into_control<B: Behavior>(self, behavior: B) -> crate::view::Control<B, R> {
    let defaults = crate::view::Control::new(Pending)
      .set_min_height(30)
      .set_padding(Inset::new(0, 12, 0, 12))
      .set_corner_radius(6)
      .set_item_alignment(AxisAlignment::Center)
      .set_color(Value::from(SystemColors::INK).when(State::Checked, SystemColors::ON_ACCENT))
      .set_background(
        Value::from(Color::TRANSPARENT)
          .when(State::Focused, SystemColors::CONTROL_HOVER)
          .when(State::Checked, SystemColors::ACCENT),
      )
      .set_ring(focus_ring())
      .set_ring_color(SystemColors::ACCENT)
      .set_label_placement(LabelPlacement::Leading);

    self.control.over(defaults).with_behavior(behavior)
  }
}

/// What a [`ListboxItem`] carries: every control's members.
pub trait ListboxItemBase<R: Realization>: Wraps<Inner = ListboxItem<R>> {
  /// An option captioned by `label`: a text names it, a view by the texts it shows or through
  /// [`Label::set_name`].
  fn new(label: impl Into<Label<R>>) -> Self {
    Self::wrap(ListboxItem {
      control: crate::view::Control::new(Pending).set_label(label),
    })
  }
}

impl<R: Realization> ListboxItemBase<R> for ListboxItem<R> {}

/// What makes a list box's option from its value and its caption.
type ItemBuilder<R> = Rc<dyn Fn(&AnyChoice, Label<R>) -> ListboxItem<R>>;

/// Options in a list, with its caption: Skene's list box, drawn on every platform. It holds
/// `C`, what it has chosen: an `Option` of its options' values for one choice, a `HashSet` or a
/// `BTreeSet` of them for several. It is a [`crate::view::Control`] over a
/// [`crate::behavior::Listbox`], its caption before it among its siblings (above it in a
/// column), a press on the caption focusing it, and its list ([`Listbox::default_list`]) of
/// options 2 pixels apart ([`ListboxBase::set_option_spacing`]), each from its item builder
/// ([`ListboxBase::set_item_builder`]) and estimated at 30 pixels tall until built
/// ([`ListboxBase::set_option_height`]). The list scrolls
/// once it is as tall as it may be, and builds only the options in view and a few beyond
/// ([`crate::view::Lazy`]), so a list of a hundred thousand costs what it shows; a reader
/// finds the whole count and each built option's place.
///
/// The list box is one tab stop, on the option focus was last on (its chosen one, else its
/// first enabled, where focus has not been in it), kept built wherever the list scrolls; the
/// arrows, the page keys, `Home` and `End` move focus without choosing, through every option
/// whether built or not, as does typing the start of an option's name; `Enter`, `Space` and a
/// click choose the focused option: alone for one choice, added or taken away for several.
/// Where several can be chosen, `Shift` and a click choose every option from the one last
/// chosen alone to the one clicked, and `Ctrl` and `A` choose every option, built or not.
///
/// It is controlled: it shows what [`ChoiceBase::derive_chosen`] returns, asks
/// [`ChoiceBase::handle_choose`] for each choice, and shows the truth.
///
/// It is thin over a [`ListboxCore`], which holds its values and its choice with their types
/// erased ([`AnyChoice`], [`AnyChosen`]), each choice made by the rules of `C`
/// ([`Chosen::choosing`]).
///
/// [`ChoiceBase::derive_chosen`]: crate::view::ChoiceBase::derive_chosen
/// [`ChoiceBase::handle_choose`]: crate::view::ChoiceBase::handle_choose
#[must_use]
pub struct Listbox<C: Chosen, R: Realization> {
  /// The list box, its values' type erased.
  core: ListboxCore<R>,
  /// What it chooses.
  chosen: PhantomData<fn(C) -> C>,
}

/// A [`Listbox`] with its values' type erased: what every list box builds.
#[must_use]
pub struct ListboxCore<R: Realization> {
  /// The control it composes, captioned, its defaults set and its behavior given as it is
  /// built.
  control: crate::view::Control<Pending, R>,
  /// What it has chosen.
  chosen: ErasedChosen,
  /// Its options' runs.
  segments: Vec<OptionSegment<R>>,
  /// The box around its options; the default where unset.
  list: Option<Container<R>>,
  /// The space between its options; 2 where unset.
  option_spacing: Option<Value<Spacing>>,
  /// How tall its options are; estimated at 30 where unset.
  option_height: Option<Value<ItemExtent>>,
  /// What makes each option; [`ListboxItem::new`] where unset.
  builder: Option<ItemBuilder<R>>,
}

/// What a list box has chosen, its type erased: one value or none, or a set.
enum ErasedChosen {
  One(Controlled<AnyChosen<false>>),
  Set(Controlled<AnyChosen<true>>),
}

impl ErasedChosen {
  /// Nothing read, each choice made by the rules of `C`, until something is.
  fn unread<C: Chosen>() -> Self {
    if C::SEVERAL {
      let mut chosen = Controlled::new();

      chosen.set(AnyChosen::unread::<C>());

      Self::Set(chosen)
    } else {
      let mut chosen = Controlled::new();

      chosen.set(AnyChosen::unread::<C>());

      Self::One(chosen)
    }
  }

  /// Takes what `typed` is read through and its handler, where it has them.
  fn merge<C: Chosen>(&mut self, typed: Controlled<C>) {
    match self {
      Self::One(chosen) => chosen.merge(typed.map(AnyChosen::new, AnyChosen::into_chosen)),
      Self::Set(chosen) => chosen.merge(typed.map(AnyChosen::new, AnyChosen::into_chosen)),
    }
  }
}

wraps_itself!(Listbox<C, R> where C: Chosen, R: Realization);

control_surfaces!([C: Chosen, R: Realization] Listbox<C, R>, R => core.control);

impl<C: Chosen, R: Realization> HasChoice<C> for Listbox<C, R> {
  fn change_chosen(&mut self, change: impl FnOnce(&mut Controlled<C>)) {
    let mut typed = Controlled::new();

    change(&mut typed);
    self.core.chosen.merge(typed);
  }
}

impl<C: Chosen, R: Realization> HasOptions<R> for Listbox<C, R> {
  fn segments_mut(&mut self) -> &mut Vec<OptionSegment<R>> {
    &mut self.core.segments
  }
}

impl<C: Chosen, R: Realization> Listbox<C, R> {
  /// The default list: [`SystemColors::FIELD`] inside a border of 1 in
  /// [`SystemColors::BORDER`], its corners rounded by 8, padded 4, 320 pixels tall at most
  /// (about ten options), its options scrolling beyond.
  pub fn default_list() -> Container<R> {
    ListboxCore::default_list()
  }
}

impl<R: Realization> ListboxCore<R> {
  /// [`Listbox::default_list`].
  pub fn default_list() -> Container<R> {
    Container::new()
      .set_background(SystemColors::FIELD)
      .set_border(Border::new(1))
      .set_border_color(SystemColors::BORDER)
      .set_corner_radius(8)
      .set_padding(Inset::all(4))
      .set_max_height(320)
  }

  /// A list box labelled `label`, with no options, nothing read of what it has `chosen`.
  fn new(label: Label<R>, chosen: ErasedChosen) -> Self {
    Self {
      control: crate::view::Control::new(Pending).set_label(label),
      chosen,
      segments: Vec::new(),
      list: None,
      option_spacing: None,
      option_height: None,
      builder: None,
    }
  }
}

/// What a [`Listbox`] carries beside every control's members, its choice and its options: its
/// list and what makes each option.
pub trait ListboxBase<C: Chosen, R: Realization>: Wraps<Inner = Listbox<C, R>> {
  /// A list box labelled `label`, with no options.
  fn new(label: impl Into<Label<R>>) -> Self {
    Self::wrap(Listbox {
      core: ListboxCore::new(label.into(), ErasedChosen::unread::<C>()),
      chosen: PhantomData,
    })
  }

  /// The space between the options, in pixels; 2 where unset.
  fn set_option_spacing(mut self, spacing: impl Into<Value<Spacing>>) -> Self {
    self.inner_mut().core.option_spacing = Some(spacing.into());

    self
  }

  /// The space between the options, in pixels, kept current; 2 where unset.
  fn derive_option_spacing<V: Into<Value<Spacing>>>(
    self,
    spacing: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_option_spacing(Value::derived(spacing))
  }

  /// How tall each option is: fixed (`36`), or estimated and measured as it is built
  /// (`ItemExtent::estimated(30)`, the default).
  fn set_option_height(mut self, height: impl Into<Value<ItemExtent>>) -> Self {
    self.inner_mut().core.option_height = Some(height.into());

    self
  }

  /// How tall each option is, kept current; estimated at 30 where unset.
  fn derive_option_height<V: Into<Value<ItemExtent>>>(
    self,
    height: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_option_height(Value::derived(height))
  }

  /// The box around its options, in place of [`Listbox::default_list`]; the options are
  /// placed in it, so a child it holds is replaced.
  fn set_list(mut self, list: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().core.list = Some(list.into_inner());

    self
  }

  /// Makes each option with `builder`, from its value and its caption: a styled
  /// [`ListboxItem`]; `ListboxItem::new(label)` where unset.
  fn set_item_builder<I: Wraps<Inner = ListboxItem<R>>>(
    mut self,
    builder: impl Fn(&C::Value, Label<R>) -> I + 'static,
  ) -> Self {
    self.inner_mut().core.builder = Some(Rc::new(move |value, label| {
      builder(value.value::<C::Value>(), label).into_inner()
    }));

    self
  }
}

impl<C: Chosen, R: Realization> ListboxBase<C, R> for Listbox<C, R> {}

impl<C: Chosen, R: Realization> AnyKind for Listbox<C, R> {}

impl<C: Chosen, R: Realization> Thin<R> for Listbox<C, R> {
  type Core = ListboxCore<R>;

  fn into_core(self) -> ListboxCore<R> {
    self.core
  }
}

impl<C: Chosen, R: Realization> View<R> for Listbox<C, R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    self.core.build(cx)
  }
}

impl<R: Realization> AnyKind for ListboxCore<R> {}

impl<R: Realization> View<R> for ListboxCore<R> {
  fn build(mut self, cx: &mut Cx<R>) -> Node {
    match std::mem::replace(&mut self.chosen, ErasedChosen::One(Controlled::new())) {
      ErasedChosen::One(chosen) => self.build_choosing(chosen, cx),
      ErasedChosen::Set(chosen) => self.build_choosing(chosen, cx),
    }
  }
}

impl<R: Realization> ListboxCore<R> {
  /// Builds this list box, what it has chosen `chosen`.
  fn build_choosing<const SEVERAL: bool>(
    self,
    chosen: Controlled<AnyChosen<SEVERAL>>,
    cx: &mut Cx<R>,
  ) -> Node {
    let Self {
      control,
      segments,
      list,
      option_spacing,
      option_height,
      builder,
      ..
    } = self;
    let behavior = ListboxBehavior::<AnyChosen<SEVERAL>>::new();
    let roster = behavior.roster();
    let options = GroupOptions::new(segments);
    let handle = LazyHandle::new();
    let defaults = crate::view::Control::new(Pending).set_spacing(6);
    let list = list.unwrap_or_else(ListboxCore::default_list);
    let option_spacing = option_spacing.unwrap_or_else(|| Value::from(2));
    let option_height = option_height.unwrap_or_else(|| Value::from(ItemExtent::Estimated(30.)));
    let builder: ItemBuilder<R> =
      builder.unwrap_or_else(|| Rc::new(|_, label| ListboxItem::new(label)));

    roster.change_chosen(|controlled| *controlled = chosen);
    roster.set_options(Rc::new(options.clone()));

    // An option focus moves to is built first, wherever the list is scrolled.
    roster.set_reveal({
      let (options, handle) = (options.clone(), handle.clone());

      move |value| {
        if let Some(index) = untrack(|| options.index_of(value)) {
          handle.realize(index);
        }
      }
    });

    // The option the list box's one tab stop is on stays built.
    Effect::new({
      let (options, handle, roster) = (options.clone(), handle.clone(), Rc::clone(&roster));

      move || {
        let stop = roster
          .active()
          .or_else(|| roster.stop())
          .and_then(|value| options.index_of(&value));

        untrack(|| handle.keep(stop));
      }
    });

    let items = Lazy::new(Axis::Vertical, None)
      .bare()
      .set_spacing(option_spacing)
      .set_item_height(option_height)
      .set_lazy_handle(&handle)
      .derive_items_keeping(
        {
          let options = options.clone();

          move || options.values()
        },
        Clone::clone,
        {
          let options = options.clone();

          move |value: AnyChoice| {
            let member = ListboxBehavior::<AnyChosen<SEVERAL>>::with_roster(Rc::clone(&roster))
              .member(value.clone());

            builder(&value, options.caption(&value)).into_control(member)
          }
        },
        // An option captioned by a view given once stays built once built.
        move |value| options.caption_once(value),
      );

    control
      .over(defaults)
      .with_behavior(behavior)
      .set_axis(Axis::Vertical)
      .set_child(AsPart::new(Part::List).set_child(list.set_child(items)))
      .build(cx)
  }
}
