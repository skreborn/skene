//! What a node shows that a view writes as one typed value: its text, an entry's text
//! with its selection, a toggle's checked state, a range's value within its bounds, a
//! choice's options with the chosen one.

#[cfg(debug_assertions)]
use std::cell::Ref;
use std::{fmt::Debug, marker::PhantomData, rc::Rc};

use crate::spine::{CheckState, Edit, NodeId, NodeKind, Realization};
#[cfg(debug_assertions)]
use crate::spine::{NodeRecord, Scene};

/// The name of a content a node takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Content {
  /// A run's text.
  Text,
  /// A text entry's text with its selection.
  Edit,
  /// A toggle's checked state.
  Checked,
  /// A range's value with its bounds and step.
  Range,
  /// A choice's options with the chosen one.
  Choice,
}

impl NodeKind {
  /// The contents a node of this kind takes.
  #[must_use]
  pub const fn contents(self) -> &'static [Content] {
    match self {
      Self::Run => &[Content::Text],
      Self::Control => &[
        Content::Edit,
        Content::Checked,
        Content::Range,
        Content::Choice,
      ],
      Self::Box | Self::Anchor | Self::Scope | Self::Overlay => &[],
    }
  }
}

/// A content's value as a realization receives it, borrowed from the writer: one
/// variant per [`Content`], each holding the value whole.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum ContentValue<'a> {
  /// A run's text.
  Text(&'a str),
  /// A text entry's text with its selection.
  Edit(&'a Edit),
  /// A toggle's checked state.
  Checked(CheckState),
  /// A range's value with its bounds and step.
  Range(&'a Range),
  /// A choice's options with the chosen one.
  Choice(&'a Choices),
}

impl ContentValue<'_> {
  /// The content this value is of.
  #[inline(always)]
  pub const fn content(&self) -> Content {
    match self {
      Self::Text(_) => Content::Text,
      Self::Edit(_) => Content::Edit,
      Self::Checked(_) => Content::Checked,
      Self::Range(_) => Content::Range,
      Self::Choice(_) => Content::Choice,
    }
  }
}

/// A value a content holds: what a view writes through a [`ContentKey`] of its type.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a value a node's content holds",
  label = "a content holds a text, an edit, a checked state, a range or choices",
  note = "write a content through its key in `contents`, with the value of the key's type"
)]
pub trait ContentType: Clone + PartialEq + Debug + 'static {}

impl ContentType for Rc<str> {}

impl ContentType for Edit {}

impl ContentType for CheckState {}

impl ContentType for Range {}

impl ContentType for Choices {}

/// The key a content is written by, typed by the value it holds, so a value of another
/// type does not compile.
///
/// A view writes its node's content whole through it, the realization showing every
/// part of the change at once:
///
/// ```
/// use std::rc::Rc;
///
/// use skene_foundation::{NodeId, Realization, contents};
///
/// fn label<P: Realization>(run: NodeId) {
///   contents::TEXT.write::<P>(run, &Rc::from("Saved"));
/// }
/// ```
///
/// A value of another type is refused:
///
/// ```compile_fail,E0308
/// use skene_foundation::{NodeId, Realization, contents};
///
/// fn check<P: Realization>(toggle: NodeId) {
///   contents::CHECKED.write::<P>(toggle, &1);
/// }
/// ```
#[must_use]
pub struct ContentKey<T: ContentType> {
  content: Content,
  erase: for<'a> fn(&'a T) -> ContentValue<'a>,
  value: PhantomData<fn(&T)>,
}

impl<T: ContentType> Clone for ContentKey<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self::new(self.content, self.erase)
  }
}

impl<T: ContentType> Debug for ContentKey<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_tuple("ContentKey")
      .field(&self.content)
      .finish()
  }
}

impl<T: ContentType> ContentKey<T> {
  /// The key of `content`, its value erased by `erase`.
  #[inline(always)]
  const fn new(content: Content, erase: for<'a> fn(&'a T) -> ContentValue<'a>) -> Self {
    Self {
      content,
      erase,
      value: PhantomData,
    }
  }

  /// The content the key writes.
  #[inline(always)]
  pub const fn content(&self) -> Content {
    self.content
  }

  /// `value` as the realization receives it.
  #[inline(always)]
  pub fn erase<'a>(&self, value: &'a T) -> ContentValue<'a> {
    (self.erase)(value)
  }

  /// Writes `value` as `node`'s content through `P`, which shows it whole; a node of a
  /// kind that does not take the content is a debug assertion.
  pub fn write<P: Realization>(&self, node: NodeId, value: &T) {
    #[cfg(debug_assertions)]
    if let Some(kind) = Self::kind_of::<P>(node) {
      debug_assert!(
        kind.contents().contains(&self.content),
        "a {kind:?} node takes no {:?} content",
        self.content
      );
    }

    P::write(node, self.erase(value));
  }

  /// The kind of `node`, where `P`'s scene is free to read and holds it.
  #[cfg(debug_assertions)]
  fn kind_of<P: Realization>(node: NodeId) -> Option<NodeKind> {
    P::scene().with(|cell| {
      let scene: Ref<'_, Scene<P>> = cell.try_borrow().ok()?;

      scene.record(node).map(NodeRecord::kind)
    })
  }
}

/// A range's value with its bounds and the step it snaps to, as one value: a bound
/// never meets a value it was not written with.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Range {
  /// The value.
  pub value: f32,
  /// The lowest value.
  pub minimum: f32,
  /// The highest value.
  pub maximum: f32,
  /// The step values snap to; non-positive when the range is continuous.
  pub step: f32,
}

impl Range {
  /// The range `minimum..=maximum` snapping to `step`, holding `value`.
  #[inline(always)]
  pub const fn new(value: f32, minimum: f32, maximum: f32, step: f32) -> Self {
    Self {
      value,
      minimum,
      maximum,
      step,
    }
  }

  /// Whether the range is continuous: its step is non-positive.
  #[inline(always)]
  #[must_use]
  pub const fn is_continuous(&self) -> bool {
    self.step <= 0.
  }
}

/// One option a choice offers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub struct ChoiceOption {
  /// The key choosing it reports.
  pub key: Rc<str>,
  /// The name shown for it.
  pub name: Rc<str>,
  /// Whether it cannot be chosen.
  pub disabled: bool,
}

impl ChoiceOption {
  /// The option `key`, shown as `name`.
  pub fn new(key: impl Into<Rc<str>>, name: impl Into<Rc<str>>) -> Self {
    Self {
      key: key.into(),
      name: name.into(),
      disabled: false,
    }
  }

  /// This option, which cannot be chosen where `disabled`.
  #[inline(always)]
  pub const fn set_disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;

    self
  }
}

/// A choice's options, in order, with the key of the chosen one, as one value: a chosen
/// key never meets options it was not written with.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub struct Choices {
  /// The options, in order.
  pub options: Rc<[ChoiceOption]>,
  /// The key of the chosen option; `None` with nothing chosen.
  pub chosen: Option<Rc<str>>,
}

impl Choices {
  /// `options` with the option `chosen` keys chosen.
  #[inline(always)]
  pub const fn new(options: Rc<[ChoiceOption]>, chosen: Option<Rc<str>>) -> Self {
    Self { options, chosen }
  }

  /// The chosen option, where the chosen key names one of the options.
  #[must_use]
  pub fn chosen_option(&self) -> Option<&ChoiceOption> {
    let chosen = self.chosen.as_deref()?;

    self.options.iter().find(|option| &*option.key == chosen)
  }
}

/// The key of each content.
pub mod contents {
  use std::rc::Rc;

  use super::{Choices, Content, ContentKey, ContentValue, Range};
  use crate::spine::{CheckState, Edit};

  /// A run's text.
  pub const TEXT: ContentKey<Rc<str>> =
    ContentKey::new(Content::Text, |text| ContentValue::Text(text));
  /// A text entry's text with its selection.
  pub const EDIT: ContentKey<Edit> =
    ContentKey::new(Content::Edit, |edit| ContentValue::Edit(edit));
  /// A toggle's checked state.
  pub const CHECKED: ContentKey<CheckState> =
    ContentKey::new(Content::Checked, |checked| ContentValue::Checked(*checked));
  /// A range's value with its bounds and step.
  pub const RANGE: ContentKey<Range> =
    ContentKey::new(Content::Range, |range| ContentValue::Range(range));
  /// A choice's options with the chosen one.
  pub const CHOICE: ContentKey<Choices> =
    ContentKey::new(Content::Choice, |choices| ContentValue::Choice(choices));
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::{ChoiceOption, Choices, Content, ContentValue, Range, contents};
  use crate::spine::{CheckState, Edit, NodeKind};

  /// A control takes every control content, a run its text alone, and the
  /// layout-invisible nodes none.
  #[test]
  fn each_kind_names_the_contents_it_takes() {
    assert_eq!(NodeKind::Run.contents(), [Content::Text]);
    assert_eq!(
      NodeKind::Control.contents(),
      [
        Content::Edit,
        Content::Checked,
        Content::Range,
        Content::Choice
      ]
    );

    for kind in [
      NodeKind::Box,
      NodeKind::Anchor,
      NodeKind::Scope,
      NodeKind::Overlay,
    ] {
      assert!(kind.contents().is_empty(), "{kind:?}");
    }
  }

  /// Each key erases its value whole into the variant of its content.
  #[test]
  fn a_key_erases_its_value_into_its_content() {
    let text: Rc<str> = Rc::from("hello");
    let edit = Edit::new("hi");
    let range = Range::new(5., 0., 10., 1.);
    let choices = Choices::new(Rc::from([ChoiceOption::new("a", "A")]), Some(Rc::from("a")));

    assert_eq!(contents::TEXT.erase(&text), ContentValue::Text("hello"));
    assert_eq!(contents::EDIT.erase(&edit), ContentValue::Edit(&edit));
    assert_eq!(
      contents::CHECKED.erase(&CheckState::Mixed),
      ContentValue::Checked(CheckState::Mixed)
    );
    assert_eq!(contents::RANGE.erase(&range), ContentValue::Range(&range));
    assert_eq!(
      contents::CHOICE.erase(&choices),
      ContentValue::Choice(&choices)
    );

    for (key, content) in [
      (contents::TEXT.content(), Content::Text),
      (contents::EDIT.content(), Content::Edit),
      (contents::CHECKED.content(), Content::Checked),
      (contents::RANGE.content(), Content::Range),
      (contents::CHOICE.content(), Content::Choice),
    ] {
      assert_eq!(key, content);
    }

    assert_eq!(contents::CHOICE.erase(&choices).content(), Content::Choice);
  }

  /// The chosen option is the one its key names, and a key naming none chooses none.
  #[test]
  fn the_chosen_option_is_the_one_its_key_names() {
    let options: Rc<[ChoiceOption]> = Rc::from([
      ChoiceOption::new("s", "Small"),
      ChoiceOption::new("m", "Medium").set_disabled(true),
    ]);

    assert_eq!(
      Choices::new(Rc::clone(&options), Some(Rc::from("m")))
        .chosen_option()
        .map(|option| &*option.name),
      Some("Medium")
    );
    assert_eq!(
      Choices::new(Rc::clone(&options), Some(Rc::from("l"))).chosen_option(),
      None
    );
    assert_eq!(Choices::new(options, None).chosen_option(), None);
  }

  /// A non-positive step leaves a range continuous.
  #[test]
  fn a_non_positive_step_is_continuous() {
    assert!(Range::new(0., 0., 1., 0.).is_continuous());
    assert!(Range::new(0., 0., 1., -1.).is_continuous());
    assert!(!Range::new(0., 0., 1., 0.5).is_continuous());
  }
}
