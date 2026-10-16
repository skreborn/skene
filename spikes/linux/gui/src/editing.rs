//! What every text entry and every control share across platforms: a controlled text with its
//! selection, and whether a control takes input.

use std::rc::Rc;

use skene_foundation::{
  NodeId,
  seam::{
    content::{Edit, contents},
    tree,
  },
  untrack,
};

use crate::{Active, TextSelection};

/// Keeps a control taking input or not (`Content::Enabled`), where the view said.
pub(crate) fn enable(id: NodeId, enabled: Option<Rc<dyn Fn() -> bool>>) {
  if let Some(enabled) = enabled {
    tree::bind::<Active, _>(id, contents::ENABLED, move || enabled());
  }
}

/// A control's `set_enabled` and `derive_enabled`, writing its `enabled` field.
macro_rules! enabled {
  () => {
    /// Whether it takes input; a disabled control takes no focus.
    pub fn set_enabled(self, enabled: bool) -> Self {
      self.derive_enabled(move || enabled)
    }

    /// Whether it takes input, kept current.
    pub fn derive_enabled(mut self, enabled: impl Fn() -> bool + 'static) -> Self {
      self.enabled = Some(Rc::new(enabled));

      self
    }
  };
}

pub(crate) use enabled;

/// A text entry's controlled text and selection, as its setters gave them.
#[derive(Clone)]
pub(crate) struct Editing {
  /// The text.
  pub(crate) text: Rc<dyn Fn() -> String>,
  /// What runs with each edit's text.
  pub(crate) change: Option<Rc<dyn Fn(String)>>,
  /// The selection, where it is controlled.
  pub(crate) selection: Option<Rc<dyn Fn() -> TextSelection>>,
  /// What runs with each move of the selection.
  pub(crate) select: Option<Rc<dyn Fn(TextSelection)>>,
  /// Whether the text can be selected but not changed.
  pub(crate) read_only: Option<Rc<dyn Fn() -> bool>>,
  /// What runs when the reader activates the entry.
  pub(crate) activate: Option<Rc<dyn Fn()>>,
}

impl Default for Editing {
  fn default() -> Self {
    Self {
      text: Rc::new(String::new),
      change: None,
      selection: None,
      select: None,
      read_only: None,
      activate: None,
    }
  }
}

impl Editing {
  /// Makes `id`'s text and selection controlled.
  pub(crate) fn apply(self, id: NodeId) {
    let Self {
      text,
      change,
      selection,
      select,
      read_only,
      activate,
    } = self;
    let truth = {
      let (text, selection) = (Rc::clone(&text), selection.clone());

      move || Edit {
        text: text().into(),
        selection: selection.as_ref().map(|selection| selection()),
      }
    };
    let handler = (change.is_some() || select.is_some()).then(|| {
      Box::new(move |edit: Edit| {
        let (now, selected) = untrack(|| (text(), selection.as_ref().map(|selection| selection())));

        if let Some(change) = &change
          && *now != *edit.text
        {
          change(edit.text.to_string());
        }

        if let (Some(select), Some(moved)) = (&select, edit.selection)
          && selected != Some(moved)
        {
          select(moved);
        }
      }) as Box<dyn Fn(Edit)>
    });

    tree::control::<Active, _>(id, contents::EDIT, truth, handler);

    if let Some(read_only) = read_only {
      tree::bind::<Active, _>(id, contents::READ_ONLY, move || read_only());
    }

    if let Some(activate) = activate {
      tree::handle_activate::<Active>(id, move || activate());
    }
  }
}

/// The text and selection setters of a text entry with an `editing: Editing` field, each doc
/// line naming the toolkit's own property or signal after the text (`""` on a cross-platform
/// control).
macro_rules! editing {
  (text: $text:literal, change: $change:literal, selection: $selection:literal, read_only: $read_only:literal) => {
    #[doc = concat!("The text", $text, ".")]
    pub fn set_text(self, text: impl Into<String>) -> Self {
      let text = text.into();

      self.derive_text(move || text.clone())
    }

    #[doc = concat!("The text", $text, ", kept current.")]
    pub fn derive_text<S: Into<String>>(mut self, text: impl Fn() -> S + 'static) -> Self {
      self.editing.text = ::std::rc::Rc::new(move || text().into());

      self
    }

    #[doc = concat!("Runs `change` with the whole text each edit proposes, before it shows", $change, ".")]
    ///
    /// Where the truth then is that text the toolkit applies the edit itself (caret, undo and
    /// the input method intact); otherwise the truth shows, as the smallest change, the caret
    /// mapped through it.
    pub fn handle_change(mut self, change: impl Fn(String) + 'static) -> Self {
      self.editing.change = Some(::std::rc::Rc::new(change));

      self
    }

    /// Shows `text` and writes each edit into it.
    pub fn bind_text(self, text: $crate::Signal<String>) -> Self {
      self
        .derive_text(move || text.get())
        .handle_change(move |value| text.set(value))
    }

    #[doc = concat!("The selection", $selection, ", in grapheme clusters of the text, kept current.")]
    ///
    /// The text entry's selection is then controlled, each move of it reported to
    /// [`Self::handle_selection_change`].
    pub fn derive_selection(
      mut self,
      selection: impl Fn() -> $crate::TextSelection + 'static,
    ) -> Self {
      self.editing.selection = Some(::std::rc::Rc::new(selection));

      self
    }

    #[doc = concat!("Runs `select` with each move of the caret or the selection", $selection, ", in grapheme clusters, where the selection is derived.")]
    pub fn handle_selection_change(
      mut self,
      select: impl Fn($crate::TextSelection) + 'static,
    ) -> Self {
      self.editing.select = Some(::std::rc::Rc::new(select));

      self
    }

    /// Places the selection at `selection` and writes each move into it.
    pub fn bind_selection(self, selection: $crate::Signal<$crate::TextSelection>) -> Self {
      self
        .derive_selection(move || selection.get())
        .handle_selection_change(move |moved| selection.set(moved))
    }

    #[doc = concat!("Whether the text can be selected and copied but not changed", $read_only, ".")]
    pub fn set_read_only(self, read_only: bool) -> Self {
      self.derive_read_only(move || read_only)
    }

    #[doc = concat!("Whether the text can be selected and copied but not changed", $read_only, ", kept current.")]
    pub fn derive_read_only(mut self, read_only: impl Fn() -> bool + 'static) -> Self {
      self.editing.read_only = Some(::std::rc::Rc::new(read_only));

      self
    }
  };
}

pub(crate) use editing;

/// `$view` given the text, the selection and their handlers the [`Editing`] `$editing` holds,
/// each through the view's public setter; its activation is the view's own.
macro_rules! give_editing {
  ($editing:expr, $view:expr) => {{
    let editing: $crate::editing::Editing = $editing;
    let text = editing.text;
    let mut view = $view.derive_text(move || text());

    if let Some(change) = editing.change {
      view = view.handle_change(move |text| change(text));
    }

    if let Some(selection) = editing.selection {
      view = view.derive_selection(move || selection());
    }

    if let Some(select) = editing.select {
      view = view.handle_selection_change(move |moved| select(moved));
    }

    if let Some(read_only) = editing.read_only {
      view = view.derive_read_only(move || read_only());
    }

    view
  }};
}

pub(crate) use give_editing;
