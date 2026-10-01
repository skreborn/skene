//! A choice among fixed options, drawn by the views it holds.

use std::{cell::RefCell, marker::PhantomData, rc::Rc};

use skene_reactive::{Effect, Get, Memo, Set, Signal, on_cleanup, provide_context, untrack};

use crate::{
  AnyKind,
  AnyView,
  Behavior,
  Control,
  DeclaredBoxStyle,
  Enhance,
  FlexDirection,
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
  NoChild,
  NodeHandle,
  NodeId,
  Overlay,
  Placement,
  PlainChild,
  Platform,
  Popup,
  Roster,
  Scene,
  TypeAhead,
  View,
  decoration,
  realize::Source,
};

/// The builder of a picker's face, given the chosen value.
#[must_use]
pub struct Face<P: Platform>(Box<dyn FnOnce(Memo<String>) -> AnyView<P>>);

impl<P: Platform> std::fmt::Debug for Face<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Face").finish_non_exhaustive()
  }
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
/// Controlled: the chosen value is what [`derive_value`](Self::derive_value) reads, and
/// a choice only calls [`handle_change`](Self::handle_change) with the value asked for;
/// [`bind_value`](Self::bind_value) wires both to one signal. Holds [`NoChild`] until
/// [`set_face`](Self::set_face) supplies the face; the options come from
/// [`set_options`](Self::set_options) and [`derive_options`](Self::derive_options),
/// each a [`BasicListboxOption`](crate::BasicListboxOption) at any depth.
#[must_use]
pub struct BasicPicker<P: Platform, F = NoChild> {
  face_style: Box<DeclaredBoxStyle<P>>,
  label: Option<Label<P>>,
  disabled: Option<Source<bool>>,
  placement: Placement,
  list: LayoutBox<P, PlainChild>,
  value: Option<Source<String>>,
  on_change: Option<Rc<dyn Fn(String)>>,
  face: F,
}

impl<P: Platform, F> std::fmt::Debug for BasicPicker<P, F> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("BasicPicker")
      .finish_non_exhaustive()
  }
}

impl<P: Platform> BasicPicker<P> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      face_style: Box::default(),
      label: None,
      disabled: None,
      placement: Placement::Bottom,
      list: LayoutBox::new(Layout::Flex {
        direction: FlexDirection::Column,
      }),
      value: None,
      on_change: None,
      face: NoChild,
    }
  }

  /// Sets the view drawing the control closed, built from the chosen value.
  pub fn set_face<V: View<P>>(
    self,
    face: impl FnOnce(Memo<String>) -> V + 'static,
  ) -> BasicPicker<P, Face<P>> {
    BasicPicker {
      face_style: self.face_style,
      label: self.label,
      disabled: self.disabled,
      placement: self.placement,
      list: self.list,
      value: self.value,
      on_change: self.on_change,
      face: Face(Box::new(move |value| face(value).into_any())),
    }
  }
}

impl<P: Platform> Default for BasicPicker<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform, F> BasicPicker<P, F> {
  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    &mut self.face_style
  }

  decoration! { style_mut, P }

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

  /// Sets the chosen value.
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Fixed(value.into()));

    self
  }

  /// Derives the chosen value from the signals `value` reads.
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Source::derive(value));

    self
  }

  /// Runs `action` with the value a choice asks for.
  pub fn handle_change(mut self, action: impl Fn(String) + 'static) -> Self {
    self.on_change = Some(Rc::new(action));

    self
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(self, signal: Signal<String>) -> Self {
    self
      .derive_value(move || signal.get())
      .handle_change(move |value| signal.set(value))
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
}

impl<P: Platform, F> AnyKind for BasicPicker<P, F> {}

/// What the picker's two forms take: the decoration, what each form's control carries,
/// and the face.
struct Parts<P: Platform> {
  face_style: Box<DeclaredBoxStyle<P>>,
  settings: ControlSettings,
  placement: Placement,
  face: Box<dyn FnOnce(Memo<String>) -> AnyView<P>>,
}

/// What each of the picker's forms gives its control: the caption's node with the name
/// it supplies, and the disabled setting.
struct ControlSettings {
  caption: Option<(NodeId, Option<Source<String>>)>,
  disabled: Option<Source<bool>>,
}

impl ControlSettings {
  /// `control` carrying the caption and the disabled setting.
  fn apply<P: Platform, B: Behavior<P>>(&self, mut control: Control<P, B>) -> Control<P, B> {
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
type Chooser = Rc<RefCell<Rc<dyn Fn(String)>>>;

/// What both forms share: the chosen value, the handler the form in force answers a
/// choice with, the roster the options joined, and the option views, built once.
struct Shared<P: Platform> {
  chosen: Rc<dyn Fn() -> Option<String>>,
  choose: Chooser,
  roster: Roster,
  options: NodeId,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> View<P> for BasicPicker<P, Face<P>> {
  fn build(self) -> NodeId {
    let Self {
      face_style,
      label,
      disabled,
      placement,
      list,
      value,
      on_change,
      face: Face(face),
    } = self;
    let chosen: Rc<dyn Fn() -> Option<String>> = match value {
      None => Rc::new(|| None),
      Some(Source::Fixed(value)) => Rc::new(move || Some(value.clone())),
      Some(Source::Derived(compute)) => Rc::new(move || Some(compute())),
    };
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

          move |value| {
            let choose = Rc::clone(&choose.borrow());

            choose(value);
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
      .set_fallback({
        let parts = Rc::clone(&parts);
        let shared = Rc::clone(&shared);
        let on_change = on_change.clone();

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
/// and disabled setting, its options read from the option views, which attach
/// nowhere; the parts stay for the drawn form.
fn build_native<P: Platform>(
  parts: &mut Option<Parts<P>>,
  shared: &Rc<Shared<P>>,
  on_change: Option<Rc<dyn Fn(String)>>,
) -> NodeId {
  let parts = parts.as_mut().expect("the native form builds first");
  let choose = move |value: String| {
    if let Some(action) = &on_change {
      action(value);
    }
  };

  *shared.choose.borrow_mut() = Rc::new(choose.clone());

  let mut control = Control::<P, _>::new(NativePicker::new().handle_change(choose));

  *control.style_mut() = (*parts.face_style).clone();

  let node = parts.settings.apply(control).build();

  shared.roster.root_at(shared.options);

  let options = untrack(|| shared.roster.options::<P>());

  P::set_options(node, &options);
  Effect::render({
    let chosen = Rc::clone(&shared.chosen);

    move |_: Option<()>| {
      let value = chosen().unwrap_or_default();

      P::set_value(node, &value);
    }
  });

  node
}

/// Builds the drawn form: the [`Popup`] face over the [`Listbox`] in its overlay.
fn build_drawn<P: Platform>(
  parts: Parts<P>,
  shared: &Rc<Shared<P>>,
  on_change: Option<Rc<dyn Fn(String)>>,
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

    Memo::new(move |_| chosen().unwrap_or_default())
  };
  let open = Signal::transient(false);
  let face_handle = NodeHandle::<P>::new();
  let listbox = Listbox::new().set_roster(shared.roster.clone());
  let choose = {
    let on_change = on_change.clone();

    move |value: String| {
      if let Some(action) = &on_change {
        action(value);
      }

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

  *control.style_mut() = *face_style;

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
      untrack(|| roster.focus_value::<P>(chosen().as_deref()));
    }
  });

  node
}

/// The closed face's type-ahead: a character typed picks among `roster`'s options by
/// name from the one `chosen` reads, and `on_change` is asked for the pick.
fn choose_typed<P: Platform>(
  roster: Roster,
  chosen: Rc<dyn Fn() -> Option<String>>,
  on_change: Option<Rc<dyn Fn(String)>>,
) -> impl Fn(&KeyEvent) -> bool {
  let type_ahead = Rc::new(TypeAhead::new());

  move |event| {
    let Some(buffer) = type_ahead.typed::<P>(event)
    else {
      return false;
    };

    if let Some(index) = roster.typed::<P>(&buffer, chosen().as_deref())
      && let Some((_, value)) = roster.enabled::<P>().get(index)
      && let Some(action) = &on_change
    {
      action(value.clone());
    }

    true
  }
}
