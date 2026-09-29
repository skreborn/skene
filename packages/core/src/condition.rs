//! The conditions a value varies under: the environment, and a node's state.

use std::hash::{Hash, Hasher};

use super::Scalar;
use crate::{
  ColorScheme,
  Size,
  composable::{Composable, ConditionShape},
};

/// The name a box publishes for container queries against its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct ContainerName(pub &'static str);

/// A test of a named container's size.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum ContainerQuery<N> {
  /// The container is at least this wide.
  MinWidth(N),
  /// The container is at most this wide.
  MaxWidth(N),
  /// The container is at least this tall.
  MinHeight(N),
  /// The container is at most this tall.
  MaxHeight(N),
}

impl<N: Scalar> ContainerQuery<N> {
  /// Whether a container of `size` satisfies the query.
  #[inline(always)]
  pub fn holds_for(&self, size: Size<N>) -> bool {
    match *self {
      Self::MinWidth(width) => size.width >= width,
      Self::MaxWidth(width) => size.width <= width,
      Self::MinHeight(height) => size.height >= height,
      Self::MaxHeight(height) => size.height <= height,
    }
  }
}

impl<N: Scalar> Hash for ContainerQuery<N> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::MinWidth(value)
      | Self::MaxWidth(value)
      | Self::MinHeight(value)
      | Self::MaxHeight(value) => state.write_u64(value.hash_bits()),
    }
  }
}

/// A condition on the environment: the viewport, the scheme, the reader's preferences,
/// or a named container's size—composable to any depth.
///
/// The one kind of condition a token's definition takes, and what a platform compiles
/// ahead of time; lengths are in the facade's scalar unit.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum Media<N> {
  /// The viewport is at least this wide.
  MinWidth(N),
  /// The viewport is at most this wide.
  MaxWidth(N),
  /// The viewport is at least this tall.
  MinHeight(N),
  /// The viewport is at most this tall.
  MaxHeight(N),
  /// The dark color scheme is in force.
  Dark,
  /// The light color scheme is in force.
  Light,
  /// The reader prefers reduced motion.
  ReducedMotion,
  /// The reader prefers reduced transparency.
  ReducedTransparency,
  /// The reader prefers more contrast.
  HighContrast,
  /// A container's size satisfies a query: the nearest container of any name without
  /// a name, the nearest one of that name with it.
  Container {
    /// The container's name; the nearest container when absent.
    name: Option<ContainerName>,
    /// The test of its size.
    query: ContainerQuery<N>,
  },
  /// Every subcondition holds; empty always holds.
  And(Vec<Self>),
  /// Any subcondition holds; empty never holds.
  Or(Vec<Self>),
}

impl<N: Scalar> Media<N> {
  /// A condition holding while every one of `conditions` holds; empty always holds.
  #[inline(always)]
  pub fn and(conditions: impl IntoIterator<Item = Self>) -> Self {
    Self::And(conditions.into_iter().collect())
  }

  /// A condition holding while any one of `conditions` holds; empty never holds.
  #[inline(always)]
  pub fn or(conditions: impl IntoIterator<Item = Self>) -> Self {
    Self::Or(conditions.into_iter().collect())
  }

  /// Whether the condition holds in `env`.
  pub fn holds_in(&self, env: &MediaEnv<'_, N>) -> bool {
    self.holds(&|leaf| leaf.leaf_holds_in(env))
  }

  fn leaf_holds_in(&self, env: &MediaEnv<'_, N>) -> bool {
    match self {
      Self::MinWidth(width) => env.environment.viewport.width >= *width,
      Self::MaxWidth(width) => env.environment.viewport.width <= *width,
      Self::MinHeight(height) => env.environment.viewport.height >= *height,
      Self::MaxHeight(height) => env.environment.viewport.height <= *height,
      Self::Dark => env.environment.scheme == ColorScheme::Dark,
      Self::Light => env.environment.scheme == ColorScheme::Light,
      Self::ReducedMotion => env.environment.reduced_motion,
      Self::ReducedTransparency => env.environment.reduced_transparency,
      Self::HighContrast => env.environment.high_contrast,
      Self::Container { name, query } => {
        (env.container)(*name).is_some_and(|size| query.holds_for(size))
      }
      Self::And(_) | Self::Or(_) => false,
    }
  }
}

impl<N: Scalar> Composable for Media<N> {
  #[inline(always)]
  fn shape(&self) -> ConditionShape<'_, Self> {
    match self {
      Self::And(subconditions) => ConditionShape::And(subconditions),
      Self::Or(subconditions) => ConditionShape::Or(subconditions),
      _ => ConditionShape::Leaf(self),
    }
  }

  #[inline(always)]
  fn dark_mode() -> Self {
    Self::Dark
  }

  #[inline(always)]
  fn both(self, other: Self) -> Self {
    Self::And(vec![self, other])
  }
}

impl<N: Scalar> Hash for Media<N> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::MinWidth(value)
      | Self::MaxWidth(value)
      | Self::MinHeight(value)
      | Self::MaxHeight(value) => state.write_u64(value.hash_bits()),
      Self::Dark
      | Self::Light
      | Self::ReducedMotion
      | Self::ReducedTransparency
      | Self::HighContrast => {}
      Self::Container { name, query } => {
        name.hash(state);
        query.hash(state);
      }
      Self::And(subconditions) | Self::Or(subconditions) => subconditions.hash(state),
    }
  }
}

/// What a platform reports of the reader's environment: every fact a [`Media`]
/// condition tests but the containers around a node.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Environment<N> {
  /// The viewport's size.
  pub viewport: Size<N>,
  /// The color scheme in force.
  pub scheme: ColorScheme,
  /// Whether the reader prefers reduced motion.
  pub reduced_motion: bool,
  /// Whether the reader prefers reduced transparency.
  pub reduced_transparency: bool,
  /// Whether the reader prefers more contrast.
  pub high_contrast: bool,
}

/// What the environment answers when a [`Media`] condition is evaluated.
#[derive(Clone, Copy)]
#[must_use]
pub struct MediaEnv<'a, N> {
  /// The reader's environment.
  pub environment: Environment<N>,
  /// The size of the nearest container of a name—of any name for `None`—when the node
  /// is inside one.
  pub container: &'a dyn Fn(Option<ContainerName>) -> Option<Size<N>>,
}

impl<N> std::fmt::Debug for MediaEnv<'_, N> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("MediaEnv").finish_non_exhaustive()
  }
}

/// A condition on a node's own interaction state, or on the state of the control it is
/// part of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum State {
  /// The pointer is over the node.
  Hovered,
  /// The node has focus.
  Focused,
  /// The node has focus that shows: reached by the keyboard, or held by a text entry,
  /// and not since a pointer press.
  FocusVisible,
  /// The node is being pressed.
  Pressed,
  /// The enclosing control is checked.
  Checked,
  /// The enclosing control is neither checked nor unchecked.
  Indeterminate,
  /// The enclosing control is disabled.
  Disabled,
  /// The enclosing control holds an invalid value.
  Invalid,
  /// The enclosing control's popup is open.
  Open,
}

impl State {
  /// Whether the state is the enclosing control's rather than the node's own.
  #[inline(always)]
  #[must_use]
  pub const fn is_control_state(self) -> bool {
    matches!(
      self,
      Self::Checked | Self::Indeterminate | Self::Disabled | Self::Invalid | Self::Open
    )
  }
}

/// A condition a property value varies under: the environment or a state, composable
/// to any depth.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub enum Condition<N> {
  /// A condition on the environment.
  Media(Media<N>),
  /// A condition on a node's state.
  State(State),
  /// Every subcondition holds; empty always holds.
  And(Vec<Self>),
  /// Any subcondition holds; empty never holds.
  Or(Vec<Self>),
}

impl<N: Scalar> Condition<N> {
  /// A condition holding while every one of `conditions` holds; empty always holds.
  #[inline(always)]
  pub fn and(conditions: impl IntoIterator<Item = Self>) -> Self {
    Self::And(conditions.into_iter().collect())
  }

  /// A condition holding while any one of `conditions` holds; empty never holds.
  #[inline(always)]
  pub fn or(conditions: impl IntoIterator<Item = Self>) -> Self {
    Self::Or(conditions.into_iter().collect())
  }

  /// Whether the condition holds in `env`, with `state` answering each state leaf.
  pub fn holds_in(&self, env: &MediaEnv<'_, N>, state: &impl Fn(State) -> bool) -> bool {
    self.holds(&|leaf| match leaf {
      Self::Media(media) => media.holds_in(env),
      Self::State(leaf) => state(*leaf),
      Self::And(_) | Self::Or(_) => false,
    })
  }

  /// Whether any leaf is a state condition.
  #[inline(always)]
  pub fn involves_state(&self) -> bool {
    self.involves(&|leaf| matches!(leaf, Self::State(_)))
  }

  /// This condition as a [`Media`] condition, where it tests the environment alone: a
  /// media leaf, or any conjunction and disjunction of them; `None` where a state is
  /// among its leaves.
  #[must_use]
  pub fn to_media(&self) -> Option<Media<N>> {
    match self {
      Self::Media(media) => Some(media.clone()),
      Self::State(_) => None,
      Self::And(subconditions) => subconditions
        .iter()
        .map(Self::to_media)
        .collect::<Option<Vec<_>>>()
        .map(Media::And),
      Self::Or(subconditions) => subconditions
        .iter()
        .map(Self::to_media)
        .collect::<Option<Vec<_>>>()
        .map(Media::Or),
    }
  }
}

impl<N: Scalar> Composable for Condition<N> {
  #[inline(always)]
  fn shape(&self) -> ConditionShape<'_, Self> {
    match self {
      Self::And(subconditions) => ConditionShape::And(subconditions),
      Self::Or(subconditions) => ConditionShape::Or(subconditions),
      _ => ConditionShape::Leaf(self),
    }
  }

  #[inline(always)]
  fn dark_mode() -> Self {
    Self::Media(Media::Dark)
  }

  #[inline(always)]
  fn both(self, other: Self) -> Self {
    Self::And(vec![self, other])
  }
}

impl<N: Scalar> Hash for Condition<N> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::Media(media) => media.hash(state),
      Self::State(leaf) => leaf.hash(state),
      Self::And(subconditions) | Self::Or(subconditions) => subconditions.hash(state),
    }
  }
}

impl<N> From<Media<N>> for Condition<N> {
  #[inline(always)]
  fn from(media: Media<N>) -> Self {
    Self::Media(media)
  }
}

impl<N> From<State> for Condition<N> {
  #[inline(always)]
  fn from(state: State) -> Self {
    Self::State(state)
  }
}

#[cfg(test)]
mod tests {
  use super::{Condition, ContainerName, ContainerQuery, Environment, Media, MediaEnv, State};
  use crate::{ColorScheme, Composable, Size};

  fn env(container: &dyn Fn(Option<ContainerName>) -> Option<Size<f32>>) -> MediaEnv<'_, f32> {
    MediaEnv {
      environment: Environment {
        viewport: Size::new(1200., 800.),
        scheme: ColorScheme::Dark,
        reduced_motion: true,
        reduced_transparency: false,
        high_contrast: false,
      },
      container,
    }
  }

  fn containers(name: Option<ContainerName>) -> Option<Size<f32>> {
    match name {
      None => Some(Size::new(300., 200.)),
      Some(ContainerName("sidebar")) => Some(Size::new(240., 800.)),
      Some(_) => None,
    }
  }

  /// A condition over the environment alone is a media condition, compound ones
  /// included; one naming a state is not.
  #[test]
  fn only_an_environment_condition_is_media() {
    let compound = Condition::<f32>::and([
      Condition::from(Media::Dark),
      Condition::or([
        Condition::from(Media::MinWidth(600.)),
        Condition::from(Media::HighContrast),
      ]),
    ]);

    assert_eq!(
      compound.to_media(),
      Some(Media::And(vec![
        Media::Dark,
        Media::Or(vec![Media::MinWidth(600.), Media::HighContrast]),
      ]))
    );
    assert_eq!(
      Condition::<f32>::and([
        Condition::from(Media::Dark),
        Condition::from(State::Hovered)
      ])
      .to_media(),
      None
    );
  }

  /// A conjunction over a disjunction expands to one conjunction per alternative.
  #[test]
  fn media_expands_to_disjunctive_normal_form() {
    let condition = Media::and([
      Media::or([Media::MinWidth(900.), Media::Dark]),
      Media::ReducedMotion,
    ]);

    assert_eq!(
      condition.disjunctive_leaves(),
      [
        vec![&Media::MinWidth(900.), &Media::ReducedMotion],
        vec![&Media::Dark, &Media::ReducedMotion],
      ]
    );
  }

  /// Every media leaf reads its answer from the environment.
  #[test]
  fn media_holds_against_the_environment() {
    let env = env(&containers);

    assert!(Media::MinWidth(1200.).holds_in(&env));
    assert!(!Media::MaxWidth(1199.).holds_in(&env));
    assert!(Media::MaxHeight(800.).holds_in(&env));
    assert!(Media::Dark.holds_in(&env));
    assert!(!Media::Light.holds_in(&env));
    assert!(Media::ReducedMotion.holds_in(&env));
    assert!(!Media::HighContrast.holds_in(&env));
    assert!(
      Media::Container {
        name: None,
        query: ContainerQuery::MinWidth(300.),
      }
      .holds_in(&env)
    );
    assert!(
      Media::Container {
        name: Some(ContainerName("sidebar")),
        query: ContainerQuery::MaxWidth(250.),
      }
      .holds_in(&env)
    );
    assert!(
      !Media::Container {
        name: Some(ContainerName("missing")),
        query: ContainerQuery::MinHeight(0.),
      }
      .holds_in(&env)
    );
    assert!(!Media::and([Media::Dark, Media::or([])]).holds_in(&env));
    assert!(Media::<f32>::and([]).holds_in(&env));
  }

  /// A condition folds media through the environment and state through the callback.
  #[test]
  fn conditions_combine_media_and_state() {
    let env = env(&containers);
    let hovered_in_dark = Condition::and([State::Hovered.into(), Media::Dark.into()]);

    assert!(hovered_in_dark.holds_in(&env, &|state| state == State::Hovered));
    assert!(!hovered_in_dark.holds_in(&env, &|_| false));
    assert!(hovered_in_dark.involves_state());
    assert!(!Condition::<f32>::from(Media::Light).involves_state());
    assert_eq!(Condition::<f32>::dark_mode(), Condition::Media(Media::Dark));
  }

  /// The control states are exactly those a plain box cannot have on its own.
  #[test]
  fn control_states_resolve_against_the_control() {
    let own = [
      State::Hovered,
      State::Focused,
      State::FocusVisible,
      State::Pressed,
    ];
    let control = [
      State::Checked,
      State::Indeterminate,
      State::Disabled,
      State::Invalid,
      State::Open,
    ];

    assert!(own.iter().all(|state| !state.is_control_state()));
    assert!(control.iter().all(|state| state.is_control_state()));
  }
}
