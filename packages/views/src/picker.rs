//! A choice among fixed options, drawn by the views it holds.

use std::rc::Rc;

use skene_core::{
  FlexDirection,
  Layout,
  Placement,
  next::{
    AnyKind,
    AnyView,
    DeclaredBoxStyle,
    Driver,
    HasChildren,
    IntoChildOf,
    IntoChildren,
    IntoView,
    NoChild,
    NodeId,
    PlainChild,
    Scene,
    View,
  },
};
use skene_reactive::{Effect, Get, Memo, Set, Signal, untrack};

use crate::{
  Anchor,
  Control,
  FocusHandle,
  Label,
  LayoutBox,
  Listbox,
  Overlay,
  Popup,
  macros::decoration,
  realize::Source,
};

/// The builder of a picker's face, given the chosen value.
#[must_use]
pub struct Face<D: Driver>(Box<dyn FnOnce(Memo<String>) -> AnyView<D>>);

/// A choice among fixed options, drawn by the views it holds: the face draws the
/// control closed, the options draw the list it opens.
///
/// The face is a [`Popup`] control decorated here, per property: a click, `Enter` or
/// `Space` toggles the list, `ArrowDown` and `ArrowUp` open it, and `Escape` closes it.
/// The list is a [`Listbox`] in a popup [`Overlay`] anchored to the face: opening
/// focuses the chosen option, or the first; the arrows move focus without wrapping;
/// `Enter`, `Space` or a click chooses, closes and returns focus to the face; a dismiss
/// closes and leaves the value alone. Type-ahead is absent.
///
/// Controlled: the chosen value is what [`derive_value`](Self::derive_value) reads, and
/// a choice only calls [`handle_change`](Self::handle_change) with the value asked for;
/// [`bind_value`](Self::bind_value) wires both to one signal. Holds [`NoChild`] until
/// [`set_face`](Self::set_face) supplies the face; the options come from
/// [`set_options`](Self::set_options) and [`derive_options`](Self::derive_options),
/// each a [`RawOption`](crate::RawOption) at any depth.
#[must_use]
pub struct RawPicker<D: Driver, F = NoChild> {
  face_style: DeclaredBoxStyle<D>,
  label: Option<Label<D>>,
  disabled: Option<Source<bool>>,
  list: LayoutBox<D, PlainChild>,
  value: Option<Source<String>>,
  on_change: Option<Rc<dyn Fn(String)>>,
  face: F,
}

impl<D: Driver> RawPicker<D> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      face_style: DeclaredBoxStyle::<D>::default(),
      label: None,
      disabled: None,
      list: LayoutBox::new(Layout::Flex {
        direction: FlexDirection::Column,
      }),
      value: None,
      on_change: None,
      face: NoChild,
    }
  }

  /// Sets the view drawing the control closed, built from the chosen value.
  #[inline(always)]
  pub fn set_face<V: IntoView<D>>(
    self,
    face: impl FnOnce(Memo<String>) -> V + 'static,
  ) -> RawPicker<D, Face<D>> {
    RawPicker {
      face_style: self.face_style,
      label: self.label,
      disabled: self.disabled,
      list: self.list,
      value: self.value,
      on_change: self.on_change,
      face: Face(Box::new(move |value| face(value).into_view())),
    }
  }
}

impl<D: Driver> Default for RawPicker<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver, F> RawPicker<D, F> {
  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<D> {
    &mut self.face_style
  }

  decoration! { style_mut }

  /// Sets the views drawing the list, each [`RawOption`](crate::RawOption) among them
  /// an option; anything else draws and nothing more.
  #[inline(always)]
  pub fn set_options(mut self, options: impl IntoChildren<D>) -> Self {
    self.list = self.list.set_children(options);

    self
  }

  /// Derives the list from a keyed collection: `items` yields the items, `key` gives
  /// each a stable identity, and `view` draws one.
  #[inline(always)]
  pub fn derive_options<T, I, K, V>(
    mut self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + std::hash::Hash + 'static,
    V: IntoChildOf<D, PlainChild>,
  {
    self.list = self.list.derive_children(items, key, view);

    self
  }

  /// Sets the chosen value.
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.value = Some(Source::Fixed(value.into()));

    self
  }

  /// Derives the chosen value from the signals `value` reads.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.value = Some(Source::derive(value));

    self
  }

  /// Runs `action` with the value a choice asks for.
  #[inline(always)]
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
  #[inline(always)]
  pub fn set_label(mut self, label: impl Into<Label<D>>) -> Self {
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
  #[inline(always)]
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Some(Source::derive(disabled));

    self
  }
}

impl<D: Driver, F> AnyKind for RawPicker<D, F> {}

impl<D: Driver> View<D> for RawPicker<D, Face<D>> {
  fn build(self) -> NodeId {
    let Self {
      face_style,
      label,
      disabled,
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
    let shown = {
      let chosen = Rc::clone(&chosen);

      Memo::new(move |_| chosen().unwrap_or_default())
    };
    let open = Signal::new(false);
    let focus = FocusHandle::<D>::new();
    let anchor = Anchor::new();
    let listbox = Listbox::new()
      .derive_chosen({
        let chosen = Rc::clone(&chosen);

        move || chosen()
      })
      .handle_choose({
        let focus = focus.clone();

        move |value| {
          if let Some(action) = &on_change {
            action(value);
          }

          open.set(false);
          focus.focus();
        }
      });
    let roster = listbox.roster().clone();
    let popup = Popup::new().derive_open(move || open.get()).handle_open({
      let focus = focus.clone();

      move |next| {
        open.set(next);

        if !next {
          focus.focus();
        }
      }
    });
    let mut control = Control::new(popup)
      .bind_anchor(&anchor)
      .bind_focus(&focus)
      .set_child_builder(move || face(shown));

    *control.style_mut() = face_style;

    if let Some(label) = label {
      control = control.set_label(label);
    }

    match disabled {
      None => {}
      Some(Source::Fixed(disabled)) => control = control.set_disabled(disabled),
      Some(Source::Derived(disabled)) => {
        control = control.derive_disabled(move || disabled());
      }
    }

    let node = control.build();
    let overlay = Overlay::<D>::popup()
      .set_anchor(&anchor)
      .set_placement(Placement::Bottom)
      .bind_open(open)
      .set_child(Control::new(listbox).set_child(list))
      .build();

    Scene::<D>::with(|scene| scene.attach(node, overlay, None));
    Effect::render(move |_: Option<()>| {
      if open.get() {
        untrack(|| roster.focus_value::<D>(chosen().as_deref()));
      }
    });

    node
  }
}
