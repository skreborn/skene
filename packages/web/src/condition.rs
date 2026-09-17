//! Conditions as CSS: the media features, container queries and selectors one
//! conjunction of leaves renders to, and the scheme's double emission.

use std::fmt::Write as _;

use skene_core::{ColorScheme, Composable, Condition, ContainerName, ContainerQuery, Media, State};

/// The class-name placeholder inside a rule template, replaced once the interner assigns
/// the content-addressed name.
pub(crate) const CLASS_PLACEHOLDER: char = '\u{1}';

/// The attribute an app's chosen color scheme is written to, on the document root.
pub const SCHEME_ATTRIBUTE: &str = "data-skene-scheme";

/// The attribute every control's root element carries.
pub const CONTROL_ATTRIBUTE: &str = "data-skene-control";

/// One conjunction of condition leaves in canonical order: what one rule is gated on.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct Conjunction {
  /// The media features, sorted; the scheme's own feature excluded.
  media: Vec<String>,
  /// The container queries by container name, sorted.
  containers: Vec<(Option<&'static str>, Vec<String>)>,
  /// The states, in a fixed order.
  states: Vec<State>,
  /// The scheme the conjunction names, if any.
  scheme: Option<ColorScheme>,
}

impl Conjunction {
  /// Every conjunction of `condition`'s disjunctive normal form, media composites
  /// inside a leaf expanded too.
  pub(crate) fn of_condition(condition: &Condition<f32>) -> Vec<Self> {
    let mut out = Vec::new();

    for term in condition.disjunctive_leaves() {
      let mut alternatives = vec![Self::default()];

      for leaf in term {
        match leaf {
          Condition::State(state) => {
            for alternative in &mut alternatives {
              alternative.push_state(*state);
            }
          }
          Condition::Media(media) => alternatives = Self::expand_media(&alternatives, media),
          Condition::And(_) | Condition::Or(_) => {}
        }
      }

      out.extend(alternatives);
    }

    out.iter_mut().for_each(Self::canonicalize);

    out
  }

  /// Every conjunction of a media condition's disjunctive normal form.
  pub(crate) fn of_media(media: &Media<f32>) -> Vec<Self> {
    let mut out = Self::expand_media(&[Self::default()], media);

    out.iter_mut().for_each(Self::canonicalize);

    out
  }

  /// The cross product of `alternatives` with the disjunctive normal form of `media`.
  fn expand_media(alternatives: &[Self], media: &Media<f32>) -> Vec<Self> {
    let terms = media.disjunctive_leaves();
    let mut out = Vec::with_capacity(alternatives.len() * terms.len());

    for alternative in alternatives {
      for term in &terms {
        let mut extended = alternative.clone();

        for leaf in term {
          extended.push_media(leaf);
        }

        out.push(extended);
      }
    }

    out
  }

  /// What a live query of this conjunction asks the environment: the media features
  /// as one query text (`None` without any), the scheme it names, and whether it has
  /// container queries, which no media query answers.
  #[cfg(skene_leaf = "browser")]
  pub(crate) fn query(&self) -> (Option<String>, Option<ColorScheme>, bool) {
    let text = (!self.media.is_empty()).then(|| self.media.join(" and "));

    (text, self.scheme, !self.containers.is_empty())
  }

  fn push_state(&mut self, state: State) {
    self.states.push(state);
  }

  fn push_media(&mut self, media: &Media<f32>) {
    match media {
      Media::MinWidth(value) => self.media.push(format!("(min-width: {value}px)")),
      Media::MaxWidth(value) => self.media.push(format!("(max-width: {value}px)")),
      Media::MinHeight(value) => self.media.push(format!("(min-height: {value}px)")),
      Media::MaxHeight(value) => self.media.push(format!("(max-height: {value}px)")),
      Media::Dark => self.scheme = Some(ColorScheme::Dark),
      Media::Light => self.scheme = Some(ColorScheme::Light),
      Media::ReducedMotion => self
        .media
        .push("(prefers-reduced-motion: reduce)".to_owned()),
      Media::ReducedTransparency => self
        .media
        .push("(prefers-reduced-transparency: reduce)".to_owned()),
      Media::HighContrast => self.media.push("(prefers-contrast: more)".to_owned()),
      Media::Container { name, query } => {
        let name = name.map(|ContainerName(name)| name);
        let query = match query {
          ContainerQuery::MinWidth(value) => format!("(min-width: {value}px)"),
          ContainerQuery::MaxWidth(value) => format!("(max-width: {value}px)"),
          ContainerQuery::MinHeight(value) => format!("(min-height: {value}px)"),
          ContainerQuery::MaxHeight(value) => format!("(max-height: {value}px)"),
        };

        match self
          .containers
          .iter_mut()
          .find(|(container, _)| *container == name)
        {
          Some((_, queries)) => queries.push(query),
          None => self.containers.push((name, vec![query])),
        }
      }
      Media::And(_) | Media::Or(_) => {}
    }
  }

  /// Sorts and deduplicates every part, so equal conditions render equal text.
  fn canonicalize(&mut self) {
    self.media.sort_unstable();
    self.media.dedup();

    for (_, queries) in &mut self.containers {
      queries.sort_unstable();
      queries.dedup();
    }

    self.containers.sort();
    self.states.sort_unstable_by_key(|state| state_rank(*state));
    self.states.dedup();
  }

  /// Appends the rule(s) holding `body` under this conjunction: the selector
  /// forms for `pseudo` on the placeholder class (or `subject` when given), wrapped in
  /// the media and container blocks, and emitted twice when a scheme is named—the
  /// preference query guarded by the deferring root, and the choice by the chosen root.
  pub(crate) fn emit(&self, out: &mut String, subject: Option<&str>, pseudo: &str, body: &str) {
    let selectors = match subject {
      Some(subject) => vec![format!("{subject}{pseudo}")],
      None => selectors(&self.states, pseudo),
    };

    match self.scheme {
      None => self.emit_wrapped(out, &self.media, "", &selectors, body),
      Some(scheme) => {
        let mut with_preference = self.media.clone();

        with_preference.push(scheme_feature(scheme).to_owned());
        with_preference.sort_unstable();
        self.emit_wrapped(
          out,
          &with_preference,
          deferring_root(scheme),
          &selectors,
          body,
        );
        self.emit_wrapped(out, &self.media, chosen_root(scheme), &selectors, body);
      }
    }
  }

  fn emit_wrapped(
    &self,
    out: &mut String,
    media: &[String],
    root: &str,
    selectors: &[String],
    body: &str,
  ) {
    let mut depth = 0;

    if !media.is_empty() {
      out.push_str("@media ");
      out.push_str(&media.join(" and "));
      out.push('{');
      depth += 1;
    }

    for (name, queries) in &self.containers {
      out.push_str("@container ");

      if let Some(name) = name {
        out.push_str(name);
        out.push(' ');
      }

      out.push_str(&queries.join(" and "));
      out.push('{');
      depth += 1;
    }

    for (index, selector) in selectors.iter().enumerate() {
      if index > 0 {
        out.push(',');
      }

      out.push_str(root);
      out.push_str(selector);
    }

    out.push('{');
    out.push_str(body);
    out.push('}');

    for _ in 0..depth {
      out.push('}');
    }
  }
}

/// The fixed order states take in a selector.
const fn state_rank(state: State) -> u8 {
  match state {
    State::Hovered => 0,
    State::Focused => 1,
    State::FocusVisible => 2,
    State::Pressed => 3,
    State::Checked => 4,
    State::Indeterminate => 5,
    State::Disabled => 6,
    State::Invalid => 7,
    State::Open => 8,
  }
}

/// How one state is observed: on the node itself, on the native element carrying a
/// control's state, or as the attribute a drawn control's root carries.
struct Facet {
  own: Option<&'static str>,
  native: Option<&'static str>,
  attribute: Option<&'static str>,
  input_only: bool,
}

const fn facet(state: State) -> Facet {
  match state {
    State::Hovered => Facet {
      own: Some(":hover"),
      native: None,
      attribute: None,
      input_only: false,
    },
    State::Focused => Facet {
      own: Some(":focus"),
      native: Some(":focus"),
      attribute: None,
      input_only: false,
    },
    State::FocusVisible => Facet {
      own: Some(":focus-visible"),
      native: Some(":focus-visible"),
      attribute: None,
      input_only: false,
    },
    State::Pressed => Facet {
      own: Some(":active"),
      native: Some(":active"),
      attribute: None,
      input_only: false,
    },
    State::Checked => Facet {
      own: None,
      native: Some(":checked"),
      attribute: Some("[data-skene-checked]"),
      input_only: true,
    },
    State::Indeterminate => Facet {
      own: None,
      native: Some(":indeterminate"),
      attribute: Some("[data-skene-indeterminate]"),
      input_only: true,
    },
    State::Disabled => Facet {
      own: Some(":disabled"),
      native: Some(":disabled"),
      attribute: Some("[data-skene-disabled]"),
      input_only: false,
    },
    State::Invalid => Facet {
      own: None,
      native: Some("[aria-invalid=\"true\"]"),
      attribute: Some("[data-skene-invalid]"),
      input_only: false,
    },
    State::Open => Facet {
      own: None,
      native: None,
      attribute: Some("[data-skene-open]"),
      input_only: false,
    },
  }
}

/// The selector forms one conjunction of states renders to, each ending in `pseudo`.
///
/// A state only the node can have (`:hover`) sits on the class. A state a control
/// carries is observed wherever the control keeps it: on the node when it is the
/// control's root (natively, by attribute, or through its own native input), on the
/// native input the node follows as a sibling part or inside one, and on the root of a
/// drawn control above it. Every anchor sits in `:where()`, so the forms share the
/// class's specificity and rule order alone decides between variants.
fn selectors(states: &[State], pseudo: &str) -> Vec<String> {
  let facets: Vec<Facet> = states.iter().copied().map(facet).collect();
  let carrier = if facets.iter().any(|facet| facet.input_only) {
    "input"
  }
  else {
    ":is(input,textarea)"
  };
  let class = |chain: &str| format!(".{CLASS_PLACEHOLDER}{chain}{pseudo}");
  let mut on_self = String::new();

  for facet in &facets {
    match (facet.native, facet.attribute) {
      (None, None) => on_self.push_str(facet.own.unwrap_or_default()),
      (native, attribute) => {
        let mut alternatives: Vec<String> = Vec::new();

        if let Some(own) = facet.own {
          alternatives.push(own.to_owned());
        }

        if let Some(native) = native {
          if facet.own != Some(native) {
            alternatives.push(native.to_owned());
          }

          alternatives.push(format!(":has(> {carrier}{native})"));
        }

        if let Some(attribute) = attribute {
          alternatives.push(attribute.to_owned());
        }

        let _ = write!(on_self, ":where(:is({}))", alternatives.join(","));
      }
    }
  }

  let mut forms = vec![class(&on_self)];
  let anchored = facets
    .iter()
    .any(|facet| facet.native.is_some() || facet.attribute.is_some());

  if !anchored {
    return forms;
  }

  if facets.iter().any(|facet| facet.native.is_some())
    && facets
      .iter()
      .all(|facet| facet.native.is_some() || facet.own.is_some())
  {
    let on_carrier: String = facets.iter().filter_map(|facet| facet.native).collect();
    let rest: String = facets
      .iter()
      .filter(|facet| facet.native.is_none())
      .filter_map(|facet| facet.own)
      .collect();

    forms.push(format!(":where({carrier}{on_carrier}) ~ {}", class(&rest)));
    forms.push(format!(
      ":where({carrier}{on_carrier} ~ *) {}",
      class(&rest)
    ));
  }

  if facets
    .iter()
    .all(|facet| facet.native.is_some() || facet.attribute.is_some() || facet.own.is_some())
  {
    let mut on_root = String::new();
    let mut rest = String::new();

    for facet in &facets {
      match (facet.native, facet.attribute) {
        (None, None) => rest.push_str(facet.own.unwrap_or_default()),
        (native, attribute) => {
          let alternatives: Vec<&str> = native.into_iter().chain(attribute).collect();

          let _ = write!(on_root, ":is({})", alternatives.join(","));
        }
      }
    }

    forms.push(format!(
      ":where([{CONTROL_ATTRIBUTE}]{on_root}) {}",
      class(&rest)
    ));
  }

  forms
}

/// The media feature a scheme renders to.
const fn scheme_feature(scheme: ColorScheme) -> &'static str {
  match scheme {
    ColorScheme::Light => "(prefers-color-scheme: light)",
    ColorScheme::Dark => "(prefers-color-scheme: dark)",
  }
}

/// The root selector under which the platform's preference still decides: any root not
/// given the *other* scheme outright.
const fn deferring_root(scheme: ColorScheme) -> &'static str {
  match scheme {
    ColorScheme::Light => ":where(:root:not([data-skene-scheme=\"dark\"])) ",
    ColorScheme::Dark => ":where(:root:not([data-skene-scheme=\"light\"])) ",
  }
}

/// The root selector under which the app has chosen this scheme outright.
const fn chosen_root(scheme: ColorScheme) -> &'static str {
  match scheme {
    ColorScheme::Light => ":where(:root[data-skene-scheme=\"light\"]) ",
    ColorScheme::Dark => ":where(:root[data-skene-scheme=\"dark\"]) ",
  }
}

#[cfg(test)]
mod tests {
  use skene_core::{Condition, ContainerName, ContainerQuery, Media, State};

  use super::{CLASS_PLACEHOLDER, Conjunction, selectors};

  fn render(condition: &Condition<f32>) -> String {
    let mut out = String::new();

    for conjunction in Conjunction::of_condition(condition) {
      conjunction.emit(&mut out, None, "", "color:red;");
    }

    out.replace(CLASS_PLACEHOLDER, "C")
  }

  /// A node's own state is a pseudo-class on the class; media wraps it, a scheme emits
  /// it twice, and a container query nests inside the media block.
  #[test]
  fn conditions_render_to_their_blocks() {
    assert_eq!(
      render(&Condition::from(State::Hovered)),
      ".C:hover{color:red;}"
    );
    assert_eq!(
      render(&Condition::and([
        Media::MinWidth(900.).into(),
        State::Hovered.into(),
        Media::Dark.into(),
      ])),
      "@media (min-width: 900px) and (prefers-color-scheme: dark)\
       {:where(:root:not([data-skene-scheme=\"light\"])) .C:hover{color:red;}}\
       @media (min-width: 900px){:where(:root[data-skene-scheme=\"dark\"]) .C:hover{color:red;}}"
    );
    assert_eq!(
      render(&Condition::from(Media::Container {
        name: Some(ContainerName("sidebar")),
        query: ContainerQuery::MinWidth(300.),
      })),
      "@container sidebar (min-width: 300px){.C{color:red;}}"
    );
  }

  /// A media composite inside a condition leaf expands into the disjunction too.
  #[test]
  fn nested_media_disjunctions_expand() {
    let condition = Condition::and([
      State::Hovered.into(),
      Media::or([Media::MinWidth(900.), Media::ReducedMotion]).into(),
    ]);

    assert_eq!(Conjunction::of_condition(&condition).len(), 2);
  }

  /// A control's state anchors on the control: the node itself as the root, the native
  /// input it follows, or the drawn root above it.
  #[test]
  fn control_states_anchor_on_the_control() {
    let forms = selectors(&[State::Checked], "");

    assert_eq!(forms.len(), 4);
    assert_eq!(
      forms[0].replace(CLASS_PLACEHOLDER, "C"),
      ".C:where(:is(:checked,:has(> input:checked),[data-skene-checked]))"
    );
    assert_eq!(
      forms[1].replace(CLASS_PLACEHOLDER, "C"),
      ":where(input:checked) ~ .C"
    );
    assert_eq!(
      forms[2].replace(CLASS_PLACEHOLDER, "C"),
      ":where(input:checked ~ *) .C"
    );
    assert_eq!(
      forms[3].replace(CLASS_PLACEHOLDER, "C"),
      ":where([data-skene-control]:is(:checked,[data-skene-checked])) .C"
    );

    let focus = selectors(&[State::FocusVisible, State::Hovered], "");

    assert_eq!(
      focus[2].replace(CLASS_PLACEHOLDER, "C"),
      ":where(:is(input,textarea):focus-visible ~ *) .C:hover"
    );
    assert_eq!(
      focus[3].replace(CLASS_PLACEHOLDER, "C"),
      ":where([data-skene-control]:is(:focus-visible)) .C:hover"
    );
  }
}
