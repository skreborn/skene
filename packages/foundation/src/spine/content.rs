//! What a node shows that a view writes as one typed value: its text, an entry's text
//! with its selection, a toggle's checked state, a range's value within its bounds, a
//! choice's options with the chosen one, a group's chosen key and a member's place in
//! its group, a link's destination, an entry's placeholder, whether it is secure and the
//! keyboard it asks for, an overlay's placement. A
//! content a behavior controls is answered by its [`Controller`], or an entry's by its
//! [`EditController`], when a reader changes it.

#[cfg(debug_assertions)]
use std::cell::Ref;
use std::{fmt::Debug, marker::PhantomData, rc::Rc};

pub use self::report::{
  Asked,
  Controllable,
  Controller,
  Controls,
  EditController,
  EditTruth,
  Report,
  report,
};
use crate::spine::{CheckState, Edit, Keyboard, NodeId, NodeKind, Placement, Point, Realization};
#[cfg(debug_assertions)]
use crate::spine::{NodeRecord, Scene};

mod report;

/// The name of a content a node takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
  /// The key of the member a group has chosen, or none.
  Chosen,
  /// The group a member belongs to, with its key there.
  Membership,
  /// The path a link navigates to when followed.
  Destination,
  /// The hint a text entry shows while its value is empty.
  Placeholder,
  /// Whether a text entry is secure: its text masked, and kept from the clipboard.
  Secure,
  /// The keyboard a text entry asks the platform for.
  Keyboard,
  /// Where an overlay sits, or the point it is pinned at.
  Placement,
}

impl NodeKind {
  /// The contents a node of this kind takes.
  #[must_use]
  pub const fn contents(self) -> &'static [Content] {
    match self {
      Self::Box => &[Content::Destination],
      Self::Run => &[Content::Text, Content::Destination],
      Self::Control => &[
        Content::Edit,
        Content::Checked,
        Content::Range,
        Content::Choice,
        Content::Chosen,
        Content::Membership,
        Content::Placeholder,
        Content::Secure,
        Content::Keyboard,
      ],
      Self::Overlay => &[Content::Placement],
      Self::Anchor | Self::Scope => &[],
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
  /// The key of the member a group has chosen, or none.
  Chosen(Option<&'a str>),
  /// The group a member belongs to, with its key there.
  Membership(&'a Membership),
  /// The path a link navigates to when followed.
  Destination(&'a str),
  /// The hint a text entry shows while its value is empty.
  Placeholder(&'a str),
  /// Whether a text entry is secure: its text masked, and kept from the clipboard.
  Secure(bool),
  /// The keyboard a text entry asks the platform for.
  Keyboard(Keyboard),
  /// Where an overlay sits, or the point it is pinned at.
  Placement(OverlayPlacement),
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
      Self::Chosen(_) => Content::Chosen,
      Self::Membership(_) => Content::Membership,
      Self::Destination(_) => Content::Destination,
      Self::Placeholder(_) => Content::Placeholder,
      Self::Secure(_) => Content::Secure,
      Self::Keyboard(_) => Content::Keyboard,
      Self::Placement(_) => Content::Placement,
    }
  }
}

/// A value a content holds: what a view writes through a [`ContentKey`] of its type.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a value a node's content holds",
  label = "a content holds a text, an edit, a checked state, a range, choices, a chosen key, a membership, a flag, a keyboard or an overlay's placement",
  note = "write a content through its key in `contents`, with the value of the key's type"
)]
pub trait ContentType: Clone + PartialEq + Debug + 'static {}

impl ContentType for Rc<str> {}

impl ContentType for Edit {}

impl ContentType for CheckState {}

impl ContentType for Range {}

impl ContentType for Choices {}

impl ContentType for Option<Rc<str>> {}

impl ContentType for Membership {}

impl ContentType for bool {}

impl ContentType for Keyboard {}

impl ContentType for OverlayPlacement {}

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
  own: fn(ContentValue<'_>) -> Option<T>,
  value: PhantomData<fn(&T)>,
}

impl<T: ContentType> Clone for ContentKey<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self::new(self.content, self.erase, self.own)
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
  /// The key of `content`, its value erased by `erase` and owned again by `own`.
  #[inline(always)]
  const fn new(
    content: Content,
    erase: for<'a> fn(&'a T) -> ContentValue<'a>,
    own: fn(ContentValue<'_>) -> Option<T>,
  ) -> Self {
    Self {
      content,
      erase,
      own,
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

  /// `value` as the key's type, where it is of the key's content.
  #[inline(always)]
  #[must_use]
  pub fn own(&self, value: ContentValue<'_>) -> Option<T> {
    (self.own)(value)
  }

  /// What `node` shows of the content as `P` reads it; `None` where `P` reads none.
  #[must_use]
  pub fn read<P: Realization>(&self, node: NodeId) -> Option<T> {
    P::read(node, self.content, self.own).flatten()
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

/// The group a member belongs to, with the key it goes by there, as one value: a radio
/// carries it, and its group's [`Chosen`](Content::Chosen) key checks the member whose
/// key it is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub struct Membership {
  /// The group's node.
  pub group: NodeId,
  /// The key the member goes by in its group.
  pub key: Rc<str>,
}

impl Membership {
  /// The member of `group` going by `key`.
  #[inline(always)]
  pub const fn new(group: NodeId, key: Rc<str>) -> Self {
    Self { group, key }
  }
}

/// Where an overlay sits, or the point it is pinned at, as one value: a pinned overlay
/// ignores its placement and its anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct OverlayPlacement {
  /// Where the overlay sits; anchored, `Bottom` and `Top` open below and above the
  /// anchor.
  pub placement: Placement,
  /// The point the open overlay's top-start corner is pinned to, in the units a pointer
  /// event reports, shifted to stay within the viewport; `None` where it is not pinned.
  pub position: Option<Point<f32>>,
}

impl OverlayPlacement {
  /// Sitting where `placement` says, pinned to `position` where one is given.
  #[inline(always)]
  pub const fn new(placement: Placement, position: Option<Point<f32>>) -> Self {
    Self {
      placement,
      position,
    }
  }
}

/// The key of each content.
pub mod contents {
  use std::rc::Rc;

  use super::{Choices, Content, ContentKey, ContentValue, Membership, OverlayPlacement, Range};
  use crate::spine::{CheckState, Edit, Keyboard};

  /// A run's text.
  pub const TEXT: ContentKey<Rc<str>> = ContentKey::new(
    Content::Text,
    |text| ContentValue::Text(text),
    |value| match value {
      ContentValue::Text(text) => Some(Rc::from(text)),
      _ => None,
    },
  );
  /// A text entry's text with its selection.
  pub const EDIT: ContentKey<Edit> = ContentKey::new(
    Content::Edit,
    |edit| ContentValue::Edit(edit),
    |value| match value {
      ContentValue::Edit(edit) => Some(edit.clone()),
      _ => None,
    },
  );
  /// A toggle's checked state.
  pub const CHECKED: ContentKey<CheckState> = ContentKey::new(
    Content::Checked,
    |checked| ContentValue::Checked(*checked),
    |value| match value {
      ContentValue::Checked(checked) => Some(checked),
      _ => None,
    },
  );
  /// A range's value with its bounds and step.
  pub const RANGE: ContentKey<Range> = ContentKey::new(
    Content::Range,
    |range| ContentValue::Range(range),
    |value| match value {
      ContentValue::Range(range) => Some(*range),
      _ => None,
    },
  );
  /// A choice's options with the chosen one.
  pub const CHOICE: ContentKey<Choices> = ContentKey::new(
    Content::Choice,
    |choices| ContentValue::Choice(choices),
    |value| match value {
      ContentValue::Choice(choices) => Some(choices.clone()),
      _ => None,
    },
  );
  /// The key of the member a group has chosen, or none.
  pub const CHOSEN: ContentKey<Option<Rc<str>>> = ContentKey::new(
    Content::Chosen,
    |chosen| ContentValue::Chosen(chosen.as_deref()),
    |value| match value {
      ContentValue::Chosen(chosen) => Some(chosen.map(Rc::from)),
      _ => None,
    },
  );
  /// The group a member belongs to, with its key there.
  pub const MEMBERSHIP: ContentKey<Membership> = ContentKey::new(
    Content::Membership,
    |membership| ContentValue::Membership(membership),
    |value| match value {
      ContentValue::Membership(membership) => Some(membership.clone()),
      _ => None,
    },
  );
  /// The path a box or a run that is a link navigates to when followed.
  pub const DESTINATION: ContentKey<Rc<str>> = ContentKey::new(
    Content::Destination,
    |path| ContentValue::Destination(path),
    |value| match value {
      ContentValue::Destination(path) => Some(Rc::from(path)),
      _ => None,
    },
  );
  /// The hint a text entry shows while its value is empty.
  pub const PLACEHOLDER: ContentKey<Rc<str>> = ContentKey::new(
    Content::Placeholder,
    |hint| ContentValue::Placeholder(hint),
    |value| match value {
      ContentValue::Placeholder(hint) => Some(Rc::from(hint)),
      _ => None,
    },
  );
  /// Whether a text entry is secure: its text masked, and kept from the clipboard.
  pub const SECURE: ContentKey<bool> = ContentKey::new(
    Content::Secure,
    |secure| ContentValue::Secure(*secure),
    |value| match value {
      ContentValue::Secure(secure) => Some(secure),
      _ => None,
    },
  );
  /// The keyboard a text entry asks the platform for.
  pub const KEYBOARD: ContentKey<Keyboard> = ContentKey::new(
    Content::Keyboard,
    |keyboard| ContentValue::Keyboard(*keyboard),
    |value| match value {
      ContentValue::Keyboard(keyboard) => Some(keyboard),
      _ => None,
    },
  );
  /// Where an overlay sits, or the point it is pinned at.
  pub const PLACEMENT: ContentKey<OverlayPlacement> = ContentKey::new(
    Content::Placement,
    |placement| ContentValue::Placement(*placement),
    |value| match value {
      ContentValue::Placement(placement) => Some(placement),
      _ => None,
    },
  );
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use super::{
    ChoiceOption,
    Choices,
    Content,
    ContentValue,
    Membership,
    OverlayPlacement,
    Range,
    contents,
  };
  use crate::spine::{Autofill, CheckState, Edit, Keyboard, NodeId, NodeKind, Placement, Point};

  /// A control takes every control content, a run its text and its destination, a box
  /// its destination, an overlay its placement, and the layout-invisible nodes none.
  #[test]
  fn each_kind_names_the_contents_it_takes() {
    assert_eq!(NodeKind::Box.contents(), [Content::Destination]);
    assert_eq!(
      NodeKind::Run.contents(),
      [Content::Text, Content::Destination]
    );
    assert_eq!(
      NodeKind::Control.contents(),
      [
        Content::Edit,
        Content::Checked,
        Content::Range,
        Content::Choice,
        Content::Chosen,
        Content::Membership,
        Content::Placeholder,
        Content::Secure,
        Content::Keyboard,
      ]
    );
    assert_eq!(NodeKind::Overlay.contents(), [Content::Placement]);

    for kind in [NodeKind::Anchor, NodeKind::Scope] {
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

    let path: Rc<str> = Rc::from("/docs");
    let pinned = OverlayPlacement::new(Placement::Top, Some(Point::new(4., 2.)));

    assert_eq!(
      contents::DESTINATION.erase(&path),
      ContentValue::Destination("/docs")
    );
    assert_eq!(
      contents::PLACEHOLDER.erase(&path),
      ContentValue::Placeholder("/docs")
    );
    assert_eq!(
      contents::PLACEMENT.erase(&pinned),
      ContentValue::Placement(pinned)
    );

    let chosen = Some(Rc::from("m"));
    let membership = Membership::new(NodeId::new(3, 1), Rc::from("m"));

    assert_eq!(
      contents::CHOSEN.erase(&chosen),
      ContentValue::Chosen(Some("m"))
    );
    assert_eq!(contents::CHOSEN.erase(&None), ContentValue::Chosen(None));
    assert_eq!(
      contents::MEMBERSHIP.erase(&membership),
      ContentValue::Membership(&membership)
    );

    let keyboard = Keyboard::EMAIL.with_autofill(Autofill::Email);

    assert_eq!(contents::SECURE.erase(&true), ContentValue::Secure(true));
    assert_eq!(
      contents::KEYBOARD.erase(&keyboard),
      ContentValue::Keyboard(keyboard)
    );

    for (key, content) in [
      (contents::TEXT.content(), Content::Text),
      (contents::EDIT.content(), Content::Edit),
      (contents::CHECKED.content(), Content::Checked),
      (contents::RANGE.content(), Content::Range),
      (contents::CHOICE.content(), Content::Choice),
      (contents::CHOSEN.content(), Content::Chosen),
      (contents::MEMBERSHIP.content(), Content::Membership),
      (contents::DESTINATION.content(), Content::Destination),
      (contents::PLACEHOLDER.content(), Content::Placeholder),
      (contents::SECURE.content(), Content::Secure),
      (contents::KEYBOARD.content(), Content::Keyboard),
      (contents::PLACEMENT.content(), Content::Placement),
    ] {
      assert_eq!(key, content);
    }

    assert_eq!(contents::CHOICE.erase(&choices).content(), Content::Choice);
  }

  /// Each key owns again what it erases, and nothing of another content.
  #[test]
  fn a_key_owns_what_it_erases() {
    let edit = Edit::new("hi");
    let range = Range::new(5., 0., 10., 1.);

    assert_eq!(
      contents::EDIT.own(contents::EDIT.erase(&edit)),
      Some(edit.clone())
    );
    assert_eq!(
      contents::RANGE.own(contents::RANGE.erase(&range)),
      Some(range)
    );
    assert_eq!(
      contents::TEXT.own(ContentValue::Text("a")).as_deref(),
      Some("a")
    );
    assert_eq!(contents::TEXT.own(ContentValue::Placeholder("a")), None);
    assert_eq!(contents::CHECKED.own(ContentValue::Edit(&edit)), None);
    assert_eq!(contents::SECURE.own(ContentValue::Secure(true)), Some(true));
    assert_eq!(
      contents::KEYBOARD.own(ContentValue::Keyboard(Keyboard::URL)),
      Some(Keyboard::URL)
    );
    assert_eq!(
      contents::SECURE.own(ContentValue::Checked(CheckState::On)),
      None
    );
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
