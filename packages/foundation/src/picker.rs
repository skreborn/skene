//! A choice among fixed options, drawn by the views it holds.

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use skene_reactive::{Effect, Get, Memo, Set, Signal, on_cleanup, provide_context, untrack};

use crate::{
  AnyChosen,
  AnyKind,
  AnyView,
  Axis,
  Behavior,
  Choice,
  Choices,
  Chosen,
  Control,
  ControlledChoice,
  DeclaredBoxStyle,
  DecorationBase,
  Enhance,
  HasBoxStyle,
  HasChildren,
  IntoChildOf,
  IntoChildren,
  KeyEvent,
  Label,
  Layout,
  LayoutBox,
  Listbox,
  ListboxContext,
  NativePicker,
  NodeHandle,
  NodeId,
  Overlay,
  Placement,
  PlainChild,
  Popup,
  Realization,
  Roster,
  Scene,
  Thin,
  TypeAhead,
  View,
  Wraps,
  empty,
  realize::Source,
};

struct Face<P: Realization>(Box<dyn FnOnce(Memo<AnyChosen>) -> AnyView<P>>);

/// A choice among fixed options, drawn by the views it holds, with its type erased:
/// what every [`BasicPicker`] builds, the typed picker thin over it.
///
/// It is the [`BasicPicker`] it describes, holding what is chosen as an
/// [`AnyChosen`]: [`set_chosen`](Self::set_chosen) and its kin take a [`Chosen`] of the
/// options' values, an `Option<T>`, and its face is given what is chosen erased.
#[must_use]
pub struct BasicPickerCore<P: Realization> {
  face_style: Box<DeclaredBoxStyle<P>>,
  label: Option<Label<P>>,
  disabled: Option<Source<bool>>,
  placement: Placement,
  list: LayoutBox<P, PlainChild>,
  chosen: ControlledChoice,
  face: Option<Face<P>>,
}

/// A choice among fixed options, drawn by the views it holds: the face draws the
/// control closed, the options draw the list it opens.
///
/// The face is a [`Popup`] control decorated here, per property: a click, `Enter` or
/// `Space` toggles the list, `ArrowDown` and `ArrowUp` open it, and `Escape` closes it.
/// The list is a [`Listbox`] in a popup [`Overlay`] anchored to the face: opening
/// focuses the chosen option, or the first; the arrows move focus without wrapping;
/// `Enter`, `Space` or a click chooses, closes and returns focus to the face; a dismiss
/// closes and leaves the value alone. Typing over the closed face chooses the option
/// whose name the characters begin, a character repeated cycling through the options
/// starting with it; typing over the open list moves focus the same way.
///
/// Controlled: the chosen value, a value of the app's own type `T`, is what
/// [`derive_chosen`](BasicPickerBase::derive_chosen) reads, and a choice only calls
/// [`handle_choose`](BasicPickerBase::handle_choose) with the value asked for;
/// [`bind_chosen`](BasicPickerBase::bind_chosen) wires both to one signal. The face
/// comes from [`set_face`](BasicPickerBase::set_face), and a picker without one renders
/// nothing; the options come from [`set_options`](BasicPickerBase::set_options) and
/// [`derive_options`](BasicPickerBase::derive_options), each a
/// [`BasicListboxOption`](crate::BasicListboxOption) at any depth tagged with a `T`.
/// The picker is thin over a [`BasicPickerCore`], which holds its values with their
/// type erased.
#[must_use]
pub struct BasicPicker<P: Realization, T: Choice> {
  core: BasicPickerCore<P>,
  value: PhantomData<fn(T) -> T>,
}

impl<P: Realization> std::fmt::Debug for BasicPickerCore<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("BasicPickerCore")
      .finish_non_exhaustive()
  }
}

impl<P: Realization, T: Choice> std::fmt::Debug for BasicPicker<P, T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("BasicPicker")
      .finish_non_exhaustive()
  }
}

impl<P: Realization> BasicPickerCore<P> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      face_style: Box::default(),
      label: None,
      disabled: None,
      placement: Placement::Bottom,
      list: LayoutBox::new(Layout::Flex {
        axis: Axis::Vertical,
        reversed: false,
      }),
      chosen: ControlledChoice::new(),
      face: None,
    }
  }
}

impl<P: Realization> Default for BasicPickerCore<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization, T: Choice> BasicPicker<P, T> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      core: BasicPickerCore::new(),
      value: PhantomData,
    }
  }
}

impl<P: Realization, T: Choice> Default for BasicPicker<P, T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> BasicPickerCore<P> {
  /// Sets where the list opens against the face; below it unless told otherwise.
  #[inline(always)]
  pub const fn set_placement(mut self, placement: Placement) -> Self {
    self.placement = placement;

    self
  }

  /// Sets the views drawing the list, each [`BasicListboxOption`](crate::BasicListboxOption) among
  /// them an option; anything else draws and nothing more.
  pub fn set_options(mut self, options: impl IntoChildren<P>) -> Self {
    self.list = self.list.set_children(options);

    self
  }

  /// Derives the list from a keyed collection: `items` yields the items, `key` gives
  /// each a stable identity, and `view` draws one.
  pub fn derive_options<T, I, K, V>(
    mut self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + std::hash::Hash + 'static,
    V: IntoChildOf<P, PlainChild>,
  {
    self.list = self.list.derive_children(items, key, view);

    self
  }

  /// Sets what is chosen.
  pub fn set_chosen<K: Chosen>(mut self, chosen: K) -> Self {
    self.chosen.set(chosen);

    self
  }

  /// Derives what is chosen from the signals `chosen` reads.
  pub fn derive_chosen<K: Chosen>(mut self, chosen: impl Fn() -> K + 'static) -> Self {
    self.chosen.derive(chosen);

    self
  }

  /// Runs `action` with what a choice asks for.
  pub fn handle_choose<K: Chosen>(mut self, action: impl Fn(K) + 'static) -> Self {
    self.chosen.handle(action);

    self
  }

  /// Binds what is chosen to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen<K: Chosen>(mut self, signal: Signal<K>) -> Self {
    self.chosen.bind(signal);

    self
  }

  /// Sets the face's caption, which is also the accessible name.
  pub fn set_label(mut self, label: impl Into<Label<P>>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets whether the picker is present but not operable.
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.disabled = Some(Source::Fixed(disabled));

    self
  }

  /// Derives whether the picker is disabled from the signals `disabled` reads.
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Some(Source::derive(disabled));

    self
  }

  /// Sets the view drawing the control closed, built from what is chosen.
  pub fn set_face<V: View<P>>(mut self, face: impl FnOnce(Memo<AnyChosen>) -> V + 'static) -> Self {
    self.face = Some(Face(Box::new(move |chosen| face(chosen).into_any())));

    self
  }
}

impl<P: Realization> HasBoxStyle<P> for BasicPickerCore<P> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    &mut self.face_style
  }
}

impl<P: Realization> DecorationBase<P> for BasicPickerCore<P> {}

impl<P: Realization> AnyKind for BasicPickerCore<P> {}

impl<P: Realization, T: Choice> BasicPicker<P, T> {
  /// This picker, its core replaced by what `map` makes of it.
  #[inline(always)]
  fn map_core(mut self, map: impl FnOnce(BasicPickerCore<P>) -> BasicPickerCore<P>) -> Self {
    self.core = map(self.core);

    self
  }
}

impl<P: Realization, T: Choice> HasBoxStyle<P> for BasicPicker<P, T> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.core.box_style_mut()
  }
}

impl<P: Realization, T: Choice> DecorationBase<P> for BasicPicker<P, T> {}

impl<P: Realization, T: Choice> AnyKind for BasicPicker<P, T> {}

impl<P: Realization, T: Choice> Thin<P> for BasicPicker<P, T> {
  type Core = BasicPickerCore<P>;

  #[inline(always)]
  fn into_core(self) -> BasicPickerCore<P> {
    self.core
  }
}

impl<P: Realization, T: Choice> View<P> for BasicPicker<P, T> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.into_core().build()
  }
}

impl<P: Realization, T: Choice> BasicPicker<P, T> {
  /// Sets where the list opens against the face; below it unless told otherwise.
  pub fn set_placement(self, placement: Placement) -> Self {
    self.map_core(|core| core.set_placement(placement))
  }

  /// Sets the views drawing the list, each
  /// [`BasicListboxOption`](crate::BasicListboxOption) among them an option tagged with
  /// a `T`; anything else draws and nothing more.
  pub fn set_options(self, options: impl IntoChildren<P>) -> Self {
    self.map_core(|core| core.set_options(options))
  }

  /// Derives the list from a keyed collection: `items` yields the items, `key` gives
  /// each a stable identity, and `view` draws one.
  pub fn derive_options<I, A, K, V>(
    self,
    items: impl Fn() -> A + 'static,
    key: impl Fn(&I) -> K + 'static,
    view: impl Fn(I) -> V + 'static,
  ) -> Self
  where
    A: IntoIterator<Item = I>,
    K: Eq + std::hash::Hash + 'static,
    V: IntoChildOf<P, PlainChild>,
  {
    self.map_core(|core| core.derive_options(items, key, view))
  }

  /// Sets the chosen value.
  pub fn set_chosen(self, value: T) -> Self {
    self.map_core(|core| core.set_chosen(Some(value)))
  }

  /// Derives the chosen value from the signals `value` reads.
  pub fn derive_chosen(self, value: impl Fn() -> T + 'static) -> Self {
    self.map_core(|core| core.derive_chosen(move || Some(value())))
  }

  /// Runs `action` with the value a choice asks for.
  pub fn handle_choose(self, action: impl Fn(T) + 'static) -> Self {
    self.map_core(|core| {
      core.handle_choose(move |chosen: Option<T>| {
        if let Some(chosen) = chosen {
          action(chosen);
        }
      })
    })
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(self, signal: Signal<T>) -> Self {
    self
      .derive_chosen(move || signal.get())
      .handle_choose(move |value| signal.set(value))
  }

  /// Sets the face's caption, which is also the accessible name.
  pub fn set_label(self, label: impl Into<Label<P>>) -> Self {
    self.map_core(|core| core.set_label(label))
  }

  /// Sets whether the picker is present but not operable.
  #[inline(always)]
  pub fn set_disabled(self, disabled: bool) -> Self {
    self.map_core(|core| core.set_disabled(disabled))
  }

  /// Derives whether the picker is disabled from the signals `disabled` reads.
  pub fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.map_core(|core| core.derive_disabled(disabled))
  }

  /// Sets the view drawing the control closed, built from the chosen value; `None` with
  /// nothing chosen.
  pub fn set_face<V: View<P>>(self, face: impl FnOnce(Memo<Option<T>>) -> V + 'static) -> Self {
    self.map_core(|core| {
      core.set_face(move |chosen: Memo<AnyChosen>| {
        face(Memo::new(move |_| {
          chosen.get().downcast_ref::<Option<T>>().cloned().flatten()
        }))
      })
    })
  }
}

/// What a facade's [`BasicPicker`] of `T`s carries beyond the face's decoration.
///
/// Its list, its value, its caption and its face.
pub trait BasicPickerBase<P: Realization, T: Choice>: Wraps<Inner = BasicPicker<P, T>> {
  /// Sets where the list opens against the face; below it unless told otherwise.
  #[must_use]
  fn set_placement(self, placement: Placement) -> Self {
    Self::wrap(self.into_inner().set_placement(placement))
  }

  /// Sets the views drawing the list, each
  /// [`BasicListboxOption`](crate::BasicListboxOption) among them an option tagged with
  /// a `T`; anything else draws and nothing more.
  #[must_use]
  fn set_options(self, options: impl IntoChildren<P>) -> Self {
    Self::wrap(self.into_inner().set_options(options))
  }

  /// Sets the chosen value.
  #[must_use]
  fn set_chosen(self, value: T) -> Self {
    Self::wrap(self.into_inner().set_chosen(value))
  }

  /// Derives the chosen value from the signals `value` reads.
  #[must_use]
  fn derive_chosen(self, value: impl Fn() -> T + 'static) -> Self {
    Self::wrap(self.into_inner().derive_chosen(value))
  }

  /// Runs `action` with the value a choice asks for.
  #[must_use]
  fn handle_choose(self, action: impl Fn(T) + 'static) -> Self {
    Self::wrap(self.into_inner().handle_choose(action))
  }

  /// Binds the chosen value to `signal` in both directions.
  #[must_use]
  fn bind_chosen(self, signal: Signal<T>) -> Self {
    Self::wrap(self.into_inner().bind_chosen(signal))
  }

  /// Sets the face's caption, which is also the accessible name.
  #[must_use]
  fn set_label(self, label: impl Into<Label<P>>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
  }

  /// Sets whether the picker is present but not operable.
  #[must_use]
  fn set_disabled(self, disabled: bool) -> Self {
    Self::wrap(self.into_inner().set_disabled(disabled))
  }

  /// Derives whether the picker is disabled from the signals `disabled` reads.
  #[must_use]
  fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self {
    Self::wrap(self.into_inner().derive_disabled(disabled))
  }

  /// Derives the list from a keyed collection: `items` yields the items, `key` gives
  /// each a stable identity, and `view` draws one.
  #[must_use]
  fn derive_options<I, A, K, V>(
    self,
    items: impl Fn() -> A + 'static,
    key: impl Fn(&I) -> K + 'static,
    view: impl Fn(I) -> V + 'static,
  ) -> Self
  where
    A: IntoIterator<Item = I>,
    K: Eq + std::hash::Hash + 'static,
    V: IntoChildOf<P, PlainChild>,
  {
    Self::wrap(self.into_inner().derive_options(items, key, view))
  }

  /// Sets the view drawing the control closed, built from the chosen value; `None` with
  /// nothing chosen.
  #[must_use]
  fn set_face<V: View<P>>(self, face: impl FnOnce(Memo<Option<T>>) -> V + 'static) -> Self {
    Self::wrap(self.into_inner().set_face(face))
  }
}

/// What the picker's two forms take: the decoration, what each form's control carries,
/// and the face.
struct Parts<P: Realization> {
  face_style: Box<DeclaredBoxStyle<P>>,
  settings: ControlSettings,
  placement: Placement,
  face: Box<dyn FnOnce(Memo<AnyChosen>) -> AnyView<P>>,
}

/// What each of the picker's forms gives its control: the caption's node with the name
/// it supplies, and the disabled setting.
struct ControlSettings {
  caption: Option<(NodeId, Option<Source<String>>)>,
  disabled: Option<Source<bool>>,
}

impl ControlSettings {
  /// `control` carrying the caption and the disabled setting.
  fn apply<P: Realization, B: Behavior<P>>(&self, mut control: Control<P, B>) -> Control<P, B> {
    if let Some((node, name)) = &self.caption {
      let label = Label::new(*node);

      control = control.set_label(match name.clone() {
        Some(Source::Fixed(name)) => label.set_name(name),
        Some(Source::Derived(name)) => label.derive_name(move || name()),
        None => label,
      });
    }

    match self.disabled.clone() {
      None => control,
      Some(Source::Fixed(disabled)) => control.set_disabled(disabled),
      Some(Source::Derived(disabled)) => control.derive_disabled(move || disabled()),
    }
  }
}

/// The handler a choice runs, replaced by the form in force.
type Chooser = Rc<RefCell<Rc<dyn Fn(AnyChosen)>>>;

/// What both forms share: what is chosen, the handler the form in force answers a
/// choice with, the roster the options joined, and the option views, built once.
struct Shared<P: Realization> {
  chosen: Rc<dyn Fn() -> AnyChosen>,
  choose: Chooser,
  roster: Roster,
  options: NodeId,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> View<P> for BasicPickerCore<P> {
  fn build(self) -> NodeId {
    let Self {
      face_style,
      label,
      disabled,
      placement,
      list,
      chosen: controlled,
      face,
    } = self;
    let Some(Face(face)) = face
    else {
      return empty::<P>().build();
    };
    let chosen: Rc<dyn Fn() -> AnyChosen> = {
      let controlled = controlled.clone();

      Rc::new(move || controlled.read())
    };
    let on_change: Rc<dyn Fn(AnyChosen)> = Rc::new(move |next| controlled.ask(next));
    let roster = Roster::new();
    let choose: Chooser = Rc::new(RefCell::new(Rc::new(|_| {})));

    // The option views build once, under the group both forms answer through, and
    // attach wherever the form in force lists them.
    provide_context(
      ListboxContext::new::<P>(
        {
          let chosen = Rc::clone(&chosen);

          move || chosen()
        },
        {
          let choose = Rc::clone(&choose);

          move |next| {
            let choose = Rc::clone(&choose.borrow());

            choose(next);
          }
        },
        roster.clone(),
      )
      .set_commit_on_tab(true),
    );

    let options = list.build();
    // The caption builds once too, and each form mounts it in turn.
    let caption = label.map(|label| {
      let (view, name) = label.into_parts();
      let node = view.build();

      on_cleanup(move || Scene::<P>::with(|scene| scene.destroy(node)));

      (node, name)
    });

    on_cleanup(move || Scene::<P>::with(|scene| scene.destroy(options)));

    let shared = Rc::new(Shared::<P> {
      chosen,
      choose,
      roster,
      options,
      platform: PhantomData,
    });
    let parts = Parts {
      face_style,
      settings: ControlSettings { caption, disabled },
      placement,
      face,
    };
    // A platform whose tree is live from the start draws the picker outright.
    let Some(hydrated) = P::hydration()
    else {
      return build_drawn::<P>(parts, &shared, on_change);
    };
    // The parts are taken by whichever form builds; the drawn one builds last.
    let parts = Rc::new(RefCell::new(Some(parts)));

    Enhance::<P>::new(hydrated)
      .set_fallback_builder({
        let parts = Rc::clone(&parts);
        let shared = Rc::clone(&shared);
        let on_change = Rc::clone(&on_change);

        move || {
          AnyView::<P>::new(move || build_native::<P>(&mut parts.borrow_mut(), &shared, on_change))
        }
      })
      .set_child_builder(move || {
        AnyView::<P>::new(move || {
          let parts = parts
            .borrow_mut()
            .take()
            .expect("the drawn form builds last");

          build_drawn::<P>(parts, &shared, on_change)
        })
      })
      .build()
  }
}

/// Builds the scriptless form: a [`NativePicker`] carrying the decoration, caption
/// and disabled setting, its choices the options read from the option views, which
/// attach nowhere, each named by the key its option goes by in the roster, with the
/// chosen option's key, none where nothing chosen is among them; the parts stay for the
/// drawn form.
fn build_native<P: Realization>(
  parts: &mut Option<Parts<P>>,
  shared: &Rc<Shared<P>>,
  on_change: Rc<dyn Fn(AnyChosen)>,
) -> NodeId {
  let parts = parts.as_mut().expect("the native form builds first");

  *shared.choose.borrow_mut() = Rc::clone(&on_change);
  shared.roster.root_at(shared.options);

  let (chosen, roster) = (Rc::clone(&shared.chosen), shared.roster.clone());
  let choices = {
    let (chosen, roster) = (Rc::clone(&chosen), roster.clone());

    move || Choices::new(Rc::from(roster.options::<P>()), roster.key_of(&chosen()))
  };
  let picker = NativePicker::new()
    .derive_choices(choices)
    .handle_choose(move |key| {
      if let Some(value) = roster.value_of(&key) {
        on_change(untrack(|| chosen()).choosing(&value));
      }
    });
  let mut control = Control::<P, _>::new(picker);

  *control.box_style_mut() = (*parts.face_style).clone();

  parts.settings.apply(control).build()
}

/// Builds the drawn form: the [`Popup`] face over the [`Listbox`] in its overlay.
fn build_drawn<P: Realization>(
  parts: Parts<P>,
  shared: &Rc<Shared<P>>,
  on_change: Rc<dyn Fn(AnyChosen)>,
) -> NodeId {
  let Parts {
    face_style,
    settings,
    placement,
    face,
  } = parts;
  let chosen = Rc::clone(&shared.chosen);
  let shown = {
    let chosen = Rc::clone(&chosen);

    Memo::new(move |_| chosen())
  };
  let open = Signal::new(false);
  let face_handle = NodeHandle::<P>::new();
  let listbox = Listbox::new().set_roster(shared.roster.clone());
  let choose = {
    let on_change = Rc::clone(&on_change);

    move |next: AnyChosen| {
      on_change(next);
      open.set(false);
      face_handle.focus();
    }
  };

  *shared.choose.borrow_mut() = Rc::new(choose);

  let roster = shared.roster.clone();
  let list_handle = NodeHandle::<P>::new();
  let popup = Popup::new()
    .derive_open(move || open.get())
    .derive_controls(move || list_handle.node())
    .handle_open(move |next| {
      open.set(next);

      if !next {
        face_handle.focus();
      }
    })
    .set_key_fallback(choose_typed::<P>(
      roster.clone(),
      Rc::clone(&chosen),
      on_change,
    ));
  let mut control = Control::new(popup).bind_handle(face_handle);

  *control.box_style_mut() = *face_style;

  let node = settings
    .apply(control)
    .set_child_builder(move || face(shown))
    .build();
  let overlay = Overlay::<P>::popup()
    .set_anchor(face_handle)
    .set_placement(placement)
    .bind_open(open)
    .set_child(
      Control::new(listbox)
        .bind_handle(list_handle)
        .set_child(shared.options),
    )
    .build();

  Scene::<P>::with(|scene| scene.attach(node, overlay, None));
  Effect::render(move |_: Option<()>| {
    if open.get() {
      untrack(|| roster.focus_chosen::<P>(&chosen()));
    }
  });

  node
}

/// The closed face's type-ahead: a character typed picks among `roster`'s options by
/// name from the one `chosen` reads, and `on_change` is asked for the pick.
fn choose_typed<P: Realization>(
  roster: Roster,
  chosen: Rc<dyn Fn() -> AnyChosen>,
  on_change: Rc<dyn Fn(AnyChosen)>,
) -> impl Fn(&KeyEvent) -> bool {
  let type_ahead = Rc::new(TypeAhead::new());

  move |event| {
    let Some(buffer) = type_ahead.typed::<P>(event)
    else {
      return false;
    };
    let current = chosen();

    if let Some(index) = roster.typed::<P>(&buffer, |value| current.contains(value))
      && let Some((_, value)) = roster.enabled::<P>().get(index)
    {
      on_change(current.choosing(value));
    }

    true
  }
}
