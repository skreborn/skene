//! What a choice group's options carry and what it has chosen: values of the app's own
//! type, erased where a group's core holds them.

use std::{
  any::Any,
  hash::{Hash, Hasher},
  marker::PhantomData,
  rc::Rc,
};

use skene_reactive::{Get, Set, Signal, untrack};

use super::Controlled;

/// What a choice group's options carry and what it has chosen: a value of the app's own
/// type, compared to find the chosen option.
#[diagnostic::on_unimplemented(
  message = "`{Self}` cannot be a choice's value",
  note = "a choice's value is `Clone`, `Eq` and `Hash`, as SwiftUI's tag is `Hashable`: derive them, or choose by an id"
)]
pub trait Choice: Clone + Eq + Hash + 'static {}

#[diagnostic::do_not_recommend]
impl<T: Clone + Eq + Hash + 'static> Choice for T {}

/// A choice's value with its type erased: what a choice group's core and its options
/// hold, the typed group thin over it.
///
/// It compares equal to another holding an equal value of the same type, and hashes as
/// the value it holds does. Erasing an `AnyChoice` keeps it as it is.
#[derive(Clone)]
#[must_use]
pub struct AnyChoice(Rc<dyn ErasedChoice>);

impl AnyChoice {
  /// `value`, its type erased.
  pub fn new<T: Choice>(value: T) -> Self {
    match (&value as &dyn Any).downcast_ref::<Self>() {
      Some(erased) => erased.clone(),
      None => Self(Rc::new(value)),
    }
  }

  /// The value, where it is a `T`.
  #[must_use]
  pub fn downcast_ref<T: Choice>(&self) -> Option<&T> {
    self.0.as_any().downcast_ref()
  }

  /// Whether the value is a `T`.
  #[inline(always)]
  #[must_use]
  pub fn is<T: Choice>(&self) -> bool {
    self.downcast_ref::<T>().is_some()
  }
}

impl PartialEq for AnyChoice {
  fn eq(&self, other: &Self) -> bool {
    self.0.eq_choice(other.0.as_any())
  }
}

impl Eq for AnyChoice {}

impl Hash for AnyChoice {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.hash_choice(state);
  }
}

impl std::fmt::Debug for AnyChoice {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("AnyChoice").finish_non_exhaustive()
  }
}

/// What an [`AnyChoice`] asks of the value it holds.
trait ErasedChoice {
  fn as_any(&self) -> &dyn Any;

  fn eq_choice(&self, other: &dyn Any) -> bool;

  fn hash_choice(&self, state: &mut dyn Hasher);
}

impl<T: Choice> ErasedChoice for T {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn eq_choice(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<T>() == Some(self)
  }

  fn hash_choice(&self, mut state: &mut dyn Hasher) {
    self.hash(&mut state);
  }
}

/// What a choice group has chosen among its options' values: one value or none
/// (`Option<T>`).
#[diagnostic::on_unimplemented(
  message = "a choice group chooses an `Option` of its options' values, not `{Self}`",
  note = "bind an `Option<T>`, its options tagged with `T`s"
)]
pub trait Chosen: Clone + PartialEq + 'static {
  /// What each option carries.
  type Value: Choice;

  /// Whether `value` is chosen.
  fn contains(&self, value: &Self::Value) -> bool;

  /// What is chosen once the reader chooses `value` from `chosen` (nothing where none
  /// reads).
  fn choosing(chosen: Option<&Self>, value: &Self::Value) -> Self;
}

impl<T: Choice> Chosen for Option<T> {
  type Value = T;

  #[inline(always)]
  fn contains(&self, value: &T) -> bool {
    self.as_ref() == Some(value)
  }

  #[inline(always)]
  fn choosing(_: Option<&Self>, value: &T) -> Self {
    Some(value.clone())
  }
}

/// What a choice group has chosen with its type erased: what its core holds, the typed
/// group thin over it.
///
/// It holds a [`Chosen`] and chooses by that type's rules, comparing equal to another
/// holding an equal choice of the same type. One read from nothing
/// ([`AnyChosen::unread`]) has chosen nothing and chooses by its type's rules still; an
/// untyped one ([`AnyChosen::untyped`]) chooses nothing.
#[derive(Clone)]
#[must_use]
pub struct AnyChosen {
  rules: &'static dyn ChosenRules,
  chosen: Option<Rc<dyn Any>>,
}

impl AnyChosen {
  /// `chosen`, its type erased.
  pub fn new<C: Chosen>(chosen: C) -> Self {
    Self {
      rules: const { &Rules::<C>(PhantomData) },
      chosen: Some(Rc::new(chosen)),
    }
  }

  /// Nothing read: nothing chosen, each choice made by the rules of `C`.
  pub const fn unread<C: Chosen>() -> Self {
    Self {
      rules: const { &Rules::<C>(PhantomData) },
      chosen: None,
    }
  }

  /// Nothing chosen and no type to choose by: a choice chooses nothing.
  #[inline(always)]
  pub const fn untyped() -> Self {
    Self {
      rules: &Untyped,
      chosen: None,
    }
  }

  /// The choice, where it is a `C`.
  #[must_use]
  pub fn downcast_ref<C: Chosen>(&self) -> Option<&C> {
    self.chosen.as_deref()?.downcast_ref()
  }

  /// Whether `value` is chosen.
  #[must_use]
  pub fn contains(&self, value: &AnyChoice) -> bool {
    self
      .chosen
      .as_deref()
      .is_some_and(|chosen| self.rules.contains(chosen, value))
  }

  /// What is chosen once the reader chooses `value` from this choice, by its type's
  /// rules; nothing where `value` is not of its options' type.
  pub fn choosing(&self, value: &AnyChoice) -> Self {
    Self {
      rules: self.rules,
      chosen: self.rules.choosing(self.chosen.as_deref(), value),
    }
  }

  /// Whether `value` is of the type its options carry; any value is where it is untyped.
  #[must_use]
  pub fn accepts(&self, value: &AnyChoice) -> bool {
    self.rules.accepts(value)
  }
}

impl Default for AnyChosen {
  #[inline(always)]
  fn default() -> Self {
    Self::untyped()
  }
}

impl PartialEq for AnyChosen {
  fn eq(&self, other: &Self) -> bool {
    match (&self.chosen, &other.chosen) {
      (Some(chosen), Some(other)) => self.rules.same(chosen.as_ref(), other.as_ref()),
      (None, None) => true,
      _ => false,
    }
  }
}

impl std::fmt::Debug for AnyChosen {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("AnyChosen").finish_non_exhaustive()
  }
}

/// What an [`AnyChosen`] asks of the type it erases: its rules.
trait ChosenRules {
  fn contains(&self, chosen: &dyn Any, value: &AnyChoice) -> bool;

  fn choosing(&self, chosen: Option<&dyn Any>, value: &AnyChoice) -> Option<Rc<dyn Any>>;

  fn accepts(&self, value: &AnyChoice) -> bool;

  fn same(&self, chosen: &dyn Any, other: &dyn Any) -> bool;
}

/// The rules of a choice with no type: nothing is chosen.
struct Untyped;

impl ChosenRules for Untyped {
  fn contains(&self, _: &dyn Any, _: &AnyChoice) -> bool {
    false
  }

  fn choosing(&self, _: Option<&dyn Any>, _: &AnyChoice) -> Option<Rc<dyn Any>> {
    None
  }

  fn accepts(&self, _: &AnyChoice) -> bool {
    true
  }

  fn same(&self, _: &dyn Any, _: &dyn Any) -> bool {
    false
  }
}

/// The rules of `C`.
struct Rules<C>(PhantomData<fn() -> C>);

impl<C: Chosen> ChosenRules for Rules<C> {
  fn contains(&self, chosen: &dyn Any, value: &AnyChoice) -> bool {
    chosen
      .downcast_ref::<C>()
      .zip(value.downcast_ref::<C::Value>())
      .is_some_and(|(chosen, value)| chosen.contains(value))
  }

  fn choosing(&self, chosen: Option<&dyn Any>, value: &AnyChoice) -> Option<Rc<dyn Any>> {
    let value = value.downcast_ref::<C::Value>()?;
    let chosen = chosen.and_then(<dyn Any>::downcast_ref::<C>);

    Some(Rc::new(C::choosing(chosen, value)))
  }

  fn accepts(&self, value: &AnyChoice) -> bool {
    value.is::<C::Value>()
  }

  fn same(&self, chosen: &dyn Any, other: &dyn Any) -> bool {
    chosen
      .downcast_ref::<C>()
      .is_some_and(|chosen| other.downcast_ref::<C>() == Some(chosen))
  }
}

/// A choice group's controlled choice with its type erased: what its user derives, and
/// the handler a choice asks, typed where they are given.
///
/// With nothing derived it reads as nothing chosen, by the rules of the type its user
/// gave last ([`AnyChosen::unread`]), or untyped where none was given. Cloning shares the
/// closures.
#[derive(Clone, Default)]
#[must_use]
pub struct ControlledChoice {
  chosen: Controlled<AnyChosen>,
  unread: AnyChosen,
}

impl std::fmt::Debug for ControlledChoice {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("ControlledChoice")
      .finish_non_exhaustive()
  }
}

impl ControlledChoice {
  /// A choice with nothing derived and no handler.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      chosen: Controlled::new(),
      unread: AnyChosen::untyped(),
    }
  }

  /// A choice with nothing derived and no handler, choosing by the rules of `C` until
  /// its user gives another type.
  #[inline(always)]
  pub const fn typed<C: Chosen>() -> Self {
    Self {
      chosen: Controlled::new(),
      unread: AnyChosen::unread::<C>(),
    }
  }

  /// Fixes the choice at `chosen`.
  pub fn set<C: Chosen>(&mut self, chosen: C) {
    self.unread = AnyChosen::unread::<C>();
    self.chosen.set(AnyChosen::new(chosen));
  }

  /// Reads the choice through `chosen`, tracking the signals it reads.
  pub fn derive<C: Chosen>(&mut self, chosen: impl Fn() -> C + 'static) {
    self.unread = AnyChosen::unread::<C>();
    self.chosen.derive(move || AnyChosen::new(chosen()));
  }

  /// Asks `action` for every choice of a `C`.
  pub fn handle<C: Chosen>(&mut self, action: impl Fn(C) + 'static) {
    self.unread = AnyChosen::unread::<C>();
    self.chosen.handle(move |chosen: AnyChosen| {
      if let Some(chosen) = chosen.downcast_ref::<C>() {
        action(chosen.clone());
      }
    });
  }

  /// Reads the choice from `signal` and asks it for every choice.
  pub fn bind<C: Chosen>(&mut self, signal: Signal<C>) {
    self.derive(move || signal.get());
    self.handle(move |chosen| signal.set(chosen));
  }

  /// The choice as it reads now, tracking what it reads.
  pub fn read(&self) -> AnyChosen {
    self.chosen.read().unwrap_or_else(|| self.unread.clone())
  }

  /// Asks for `chosen`.
  #[inline(always)]
  pub fn ask(&self, chosen: AnyChosen) {
    self.chosen.ask(chosen);
  }

  /// Asks for what is chosen once the reader chooses `value`, from the choice as it
  /// reads now, untracked.
  pub fn choose(&self, value: &AnyChoice) {
    self.ask(untrack(|| self.read()).choosing(value));
  }
}

#[cfg(test)]
mod tests {
  use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

  use super::{AnyChoice, AnyChosen};

  #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
  enum Size {
    Small,
    Large,
  }

  /// An erased value equals another of its type and value alone, hashes as its value
  /// does, and erasing it again keeps it as it is.
  #[test]
  fn an_erased_value_compares_and_hashes_as_its_value() {
    let hasher = BuildHasherDefault::<DefaultHasher>::default();
    let small = AnyChoice::new(Size::Small);

    assert_eq!(small, AnyChoice::new(Size::Small));
    assert_ne!(small, AnyChoice::new(Size::Large));
    assert_ne!(AnyChoice::new(0_u8), AnyChoice::new(0_u16));
    assert_eq!(hasher.hash_one(&small), hasher.hash_one(Size::Small));
    assert_eq!(AnyChoice::new(small.clone()), small);
    assert_eq!(small.downcast_ref::<Size>(), Some(&Size::Small));
    assert!(!small.is::<u8>());
  }

  /// A choice chooses by its type's rules, read from nothing as well, and an untyped
  /// one chooses nothing.
  #[test]
  fn a_choice_chooses_by_its_types_rules() {
    let small = AnyChoice::new(Size::Small);
    let large = AnyChoice::new(Size::Large);
    let chosen = AnyChosen::new(Some(Size::Small));

    assert!(chosen.contains(&small));
    assert!(!chosen.contains(&large));
    assert_eq!(chosen.choosing(&large), AnyChosen::new(Some(Size::Large)));
    assert_eq!(AnyChosen::unread::<Option<Size>>().choosing(&small), chosen);
    assert!(!AnyChosen::unread::<Option<Size>>().contains(&small));
    assert_eq!(
      chosen.choosing(&AnyChoice::new(1_u8)),
      AnyChosen::unread::<Option<Size>>()
    );
    assert_eq!(AnyChosen::untyped().choosing(&small), AnyChosen::untyped());
    assert!(chosen.accepts(&large) && !chosen.accepts(&AnyChoice::new(1_u8)));
    assert!(AnyChosen::untyped().accepts(&AnyChoice::new(1_u8)));
  }
}
