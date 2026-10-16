//! Compiling a record to the level a realization keeps.

use std::{cell::Cell, fmt::Write as _, rc::Rc};

use crate::{
  Condition, Definition, DefinitionLeaf, Definitions, Environment, Leaf, Media, Signal, State,
  StateSet, TokenValue, Tokens, Value,
  seam::record::{Group, Kind, Kinded, Prop, Record, Resolved, Transition},
};

/// What of a record a realization keeps unresolved, because its toolkit resolves it: one of
/// three levels, each a pixel platform's.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Keep {
  /// Nothing: every value is resolved by the shared half, states included, which the
  /// realization reports where a record varies under them (UIKit, AppKit, Android Views,
  /// WinUI). Transitions still cross as data.
  Nothing,
  /// Color tokens as display-wide named colors, each tagged with its definition's content
  /// ([`ScopeChain::tag`]) so equal definitions share a name and different ones never do;
  /// state conditions as conditions; derived leaves as per-node slots. Everything else is
  /// resolved by the shared half (a style language short of CSS's).
  ColorsAndStates,
  /// Everything: tokens as inherited custom properties (untagged, the cascade scopes them),
  /// media and state conditions as conditions, derived leaves as per-node slots (the web).
  Everything,
}

impl Keep {
  /// Whether a token reference of `kind` stays a name.
  #[must_use]
  pub const fn keeps_token(self, kind: Kind) -> bool {
    match self {
      Self::Nothing => false,
      Self::ColorsAndStates => matches!(kind, Kind::Color),
      Self::Everything => true,
    }
  }

  /// Whether a kept token is tagged with its definition's content.
  #[must_use]
  pub const fn tags_tokens(self) -> bool {
    matches!(self, Self::ColorsAndStates)
  }

  /// Whether media conditions stay conditions; otherwise the shared half answers them
  /// against the [`Environment`], and recompiles when it changes.
  #[must_use]
  pub const fn media(self) -> bool {
    matches!(self, Self::Everything)
  }

  /// Whether state conditions stay conditions; otherwise the realization reports the states
  /// the record varies under, and the shared half answers them.
  #[must_use]
  pub const fn states(self) -> bool {
    !matches!(self, Self::Nothing)
  }

  /// Whether a derived leaf becomes a per-node slot, so its changing value never reaches
  /// the address.
  #[must_use]
  pub const fn slots(self) -> bool {
    !matches!(self, Self::Nothing)
  }
}

/// A theme scope's identity, unique on its thread: a realization's bookkeeping key for the
/// definitions a scope contributes, never part of any output.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct ScopeId(pub u32);

struct ScopeLink {
  id: ScopeId,
  definitions: Signal<Rc<Definitions>>,
  tokens: Tokens,
  parent: Option<Rc<Self>>,
}

/// The theme scopes a node sits in, innermost first: what its tokens resolve through.
#[derive(Clone, Default)]
pub struct ScopeChain(Option<Rc<ScopeLink>>);

thread_local! {
  static NEXT_SCOPE: Cell<u32> = const { Cell::new(0) };
}

impl ScopeChain {
  /// No scope: every token is undefined.
  #[must_use]
  pub const fn root() -> Self {
    Self(None)
  }

  /// A scope inside this one, defining what `definitions` holds.
  #[must_use]
  pub fn scope(&self, definitions: Signal<Rc<Definitions>>) -> Self {
    let id = NEXT_SCOPE.with(|next| {
      next.set(next.get() + 1);

      ScopeId(next.get())
    });

    Self(Some(Rc::new(ScopeLink {
      id,
      definitions,
      tokens: self.tokens().scope(definitions),
      parent: self.0.clone(),
    })))
  }

  /// The innermost scope's identity.
  #[must_use]
  pub fn id(&self) -> Option<ScopeId> {
    self.0.as_ref().map(|link| link.id)
  }

  /// The innermost scope's definitions.
  #[must_use]
  pub fn definitions(&self) -> Option<Signal<Rc<Definitions>>> {
    self.0.as_ref().map(|link| link.definitions)
  }

  /// The chain as the foundation's [`Tokens`], to resolve through.
  #[must_use]
  pub fn tokens(&self) -> Tokens {
    self
      .0
      .as_ref()
      .map_or_else(Tokens::root, |link| link.tokens.clone())
  }

  /// Whether some scope of the chain defines `name`, subscribing the running effect to the
  /// definitions it read.
  #[must_use]
  pub fn defines(&self, name: &str) -> bool {
    Self::defining(self.0.as_ref(), name).is_some()
  }

  fn defining<'a>(mut next: Option<&'a Rc<ScopeLink>>, name: &str) -> Option<&'a Rc<ScopeLink>> {
    while let Some(link) = next {
      if link
        .definitions
        .with(|definitions| definitions.get(name).is_some())
      {
        return Some(link);
      }

      next = link.parent.as_ref();
    }

    None
  }

  /// The content tag of `name`'s definition where the chain defines it: a hash of the
  /// definition with its media variants kept and every token it names replaced by that
  /// token's own tag, resolved from the defining scope outward. Equal definitions have equal
  /// tags in any scope, on any thread, in any render; a scheme switch changes none.
  #[must_use]
  pub fn tag(&self, name: &str) -> Option<u64> {
    Self::tag_from(self.0.as_ref(), name, 0)
  }

  fn tag_from(chain: Option<&Rc<ScopeLink>>, name: &str, depth: u8) -> Option<u64> {
    let link = Self::defining(chain, name)?;
    let definition = link
      .definitions
      .with(|definitions| definitions.get(name).cloned())?;
    let leaf = |leaf: &DefinitionLeaf| match leaf {
      DefinitionLeaf::Value(value) => format!("{value:?}"),
      DefinitionLeaf::Token(token) if depth < 8 => format!(
        "@{:x}",
        Self::tag_from(Some(link), token, depth + 1).unwrap_or_default()
      ),
      DefinitionLeaf::Token(_) => String::new(),
    };
    let mut text = format!("{name}={}", leaf(&definition.base));

    for (media, variant) in &definition.variants {
      let _ = write!(text, "|{media:?}:{}", leaf(variant));
    }

    Some(fnv1a(&text))
  }
}

/// A condition a realization keeps.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum KeptCondition {
  /// A media condition, kept where [`Keep::media`] is.
  Media(Media),
  /// A state, kept where [`Keep::states`] is.
  State(State),
}

/// A leaf a realization keeps.
#[derive(Clone, PartialEq, Debug)]
pub enum KeptLeaf {
  /// A value.
  Value(Resolved),
  /// A token by name, tagged with its definition's content where [`Keep::tags_tokens`] is
  /// (`0` elsewhere).
  Token {
    /// The token's name.
    name: &'static str,
    /// Its definition's content tag.
    tag: u64,
  },
  /// The node's slot of this index, holding a derived leaf's value now.
  Slot(u16),
}

/// A value as a realization keeps it: a base leaf and the kept variants, the last holding
/// winning.
#[derive(Clone, PartialEq, Debug)]
pub struct KeptValue {
  /// The leaf in force where no kept condition holds.
  pub base: KeptLeaf,
  /// Each kept condition with the leaf it puts in force, in order.
  pub variants: Vec<(KeptCondition, KeptLeaf)>,
}

impl KeptValue {
  /// The leaf in force with exactly the states `holding`, media variants taken as not
  /// holding.
  #[must_use]
  pub fn with_states(&self, holding: StateSet) -> &KeptLeaf {
    let mut leaf = &self.base;

    for (condition, variant) in &self.variants {
      if matches!(condition, KeptCondition::State(state) if holding.contains(*state)) {
        leaf = variant;
      }
    }

    leaf
  }

  /// Whether some leaf of the value is a slot.
  #[must_use]
  pub fn has_slot(&self) -> bool {
    std::iter::once(&self.base)
      .chain(self.variants.iter().map(|(_, leaf)| leaf))
      .any(|leaf| matches!(leaf, KeptLeaf::Slot(_)))
  }
}

/// A node's style compiled to the level its realization keeps: what
/// [`crate::seam::Realization::apply_style`] receives.
#[derive(Clone, PartialEq, Debug)]
pub struct CompiledStyle {
  /// Each property with its kept value.
  pub props: Vec<(Prop, KeptValue)>,
  /// The transitions.
  pub transitions: Vec<Transition>,
  /// The node's slots, each a derived leaf's value now, where [`Keep::slots`] is.
  pub slots: Vec<KeptLeaf>,
  /// The content address of `props` and `transitions` (not the slots): a function of the
  /// style's content alone, so equal styles share one on every side of the web and in every
  /// render, and a realization keeps one rule per address.
  pub address: u64,
}

/// A rule of [`CompiledStyle::state_rules`]: the states it holds under, and the properties it
/// declares.
pub type StateRule<'a> = (StateSet, Vec<(Prop, &'a KeptLeaf)>);

impl CompiledStyle {
  /// Whether the style declares nothing.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.props.is_empty() && self.transitions.is_empty()
  }

  /// The states some kept value varies under.
  #[must_use]
  pub fn states(&self) -> StateSet {
    self
      .props
      .iter()
      .flat_map(|(_, value)| &value.variants)
      .filter_map(|(condition, _)| match condition {
        KeptCondition::State(state) => Some(*state),
        KeptCondition::Media(_) => None,
      })
      .collect()
  }

  /// The rules a realization keeping states but not media writes for a class (`.c`,
  /// `.c:hover`, `.c:hover:active`), so the foundation's last-holding-wins survives a
  /// cascade in which a rule naming more states wins and, among equals, the later one.
  ///
  /// The first rule, under no state, declares every property. A rule under a combination
  /// declares only the properties whose winner among the rules before it would be wrong, and
  /// a combination left with none is left out: states varying disjoint properties cost one
  /// rule each, not one per combination.
  #[must_use]
  pub fn state_rules(&self) -> Vec<StateRule<'_>> {
    let mut rules: Vec<StateRule<'_>> = Vec::new();

    for holding in self.states().subsets() {
      let declared: Vec<(Prop, &KeptLeaf)> = self
        .props
        .iter()
        .map(|(prop, value)| (*prop, value.with_states(holding)))
        .filter(|(prop, wanted)| {
          holding.is_empty() || Self::winner(&rules, holding, *prop) != Some(*wanted)
        })
        .collect();

      if holding.is_empty() || !declared.is_empty() {
        rules.push((holding, declared));
      }
    }

    rules
  }

  /// The leaf the cascade of `rules` gives `prop` with exactly the states `holding`.
  #[must_use]
  pub fn winner<'a>(
    rules: &[StateRule<'a>],
    holding: StateSet,
    prop: Prop,
  ) -> Option<&'a KeptLeaf> {
    rules
      .iter()
      .filter(|(states, _)| states.is_subset(holding))
      .filter_map(|(states, declared)| {
        declared
          .iter()
          .find(|(name, _)| *name == prop)
          .map(|(_, leaf)| (states.len(), *leaf))
      })
      .fold(None::<(u32, &KeptLeaf)>, |best, (rank, leaf)| match best {
        Some((best_rank, _)) if best_rank > rank => best,
        _ => Some((rank, leaf)),
      })
      .map(|(_, leaf)| leaf)
  }

  /// The value of `prop` where it is resolved and unconditioned.
  #[must_use]
  pub fn resolved(&self, prop: Prop) -> Option<&Resolved> {
    self
      .props
      .iter()
      .find(|(name, _)| *name == prop)
      .and_then(
        |(_, value)| match (&value.base, value.variants.is_empty()) {
          (KeptLeaf::Value(resolved), true) => Some(resolved),
          _ => None,
        },
      )
  }
}

/// One compilation's context: what is kept, the scopes, the states.
pub(crate) struct CompileCx<'a> {
  /// What the realization keeps unresolved.
  pub(crate) keep: Keep,
  /// The theme scopes the node sits in.
  pub(crate) scope: &'a ScopeChain,
  /// Whether a state the realization does not keep holds.
  pub(crate) state: &'a dyn Fn(State) -> bool,
  /// The node's slots so far, each a derived leaf's value now.
  pub(crate) slots: Vec<KeptLeaf>,
}

impl CompileCx<'_> {
  fn leaf<T: Kinded>(&mut self, leaf: &Leaf<T>, depth: u8) -> Option<KeptValue> {
    match leaf {
      Leaf::Fixed(value) => Some(KeptValue {
        base: KeptLeaf::Value(value.clone().erase()),
        variants: Vec::new(),
      }),
      Leaf::Token(name) => {
        let base = if !self.keep.keeps_token(T::KIND) {
          let value = self.scope.tokens().resolve(name)?;

          KeptLeaf::Value(T::from_token(&value)?.erase())
        } else if self.keep.tags_tokens() {
          KeptLeaf::Token {
            name,
            tag: self.scope.tag(name)?,
          }
        } else {
          self
            .scope
            .defines(name)
            .then_some(KeptLeaf::Token { name, tag: 0 })?
        };

        Some(KeptValue {
          base,
          variants: Vec::new(),
        })
      }
      Leaf::Derived(derive) if depth < 8 => {
        let inner = derive();
        let mut kept = self.value_at(&inner, depth + 1)?;

        if self.keep.slots() {
          let index = u16::try_from(self.slots.len()).ok()?;

          self
            .slots
            .push(std::mem::replace(&mut kept.base, KeptLeaf::Slot(index)));
        }

        Some(kept)
      }
      Leaf::Derived(_) => None,
    }
  }

  fn holds(&self, condition: &Condition) -> Option<bool> {
    match condition {
      Condition::Media(_) if self.keep.media() => None,
      Condition::Media(media) => Some(Environment::current().holds(media)),
      Condition::State(_) if self.keep.states() => None,
      Condition::State(state) => Some((self.state)(*state)),
    }
  }

  fn value_at<T: Kinded>(&mut self, value: &Value<T>, depth: u8) -> Option<KeptValue> {
    let mut kept = self.leaf(value.base(), depth)?;

    for (condition, leaf) in value.variants() {
      match self.holds(condition) {
        Some(false) => {}
        Some(true) => {
          if let Some(replacement) = self.leaf(leaf, depth) {
            kept = replacement;
          }
        }
        None => {
          let Some(replacement) = self.leaf(leaf, depth) else {
            continue;
          };
          let condition = match condition {
            Condition::Media(media) => KeptCondition::Media(*media),
            Condition::State(state) => KeptCondition::State(*state),
          };

          kept.variants.push((condition, replacement.base));
          kept.variants.extend(replacement.variants);
        }
      }
    }

    Some(kept)
  }

  /// `value` compiled; `None` where its base names a token defined nowhere above.
  pub(crate) fn value<T: Kinded>(&mut self, value: &Value<T>) -> Option<KeptValue> {
    self.value_at(value, 0)
  }
}

impl Record {
  /// The properties of the groups `include` accepts, compiled to the level `keep` keeps,
  /// `state` answering for the states not kept, subscribing the running effect to every
  /// signal, token and environment fact it read.
  pub fn compile(
    &self,
    keep: Keep,
    include: impl Fn(Group) -> bool,
    scope: &ScopeChain,
    state: &dyn Fn(State) -> bool,
  ) -> CompiledStyle {
    let mut cx = CompileCx {
      keep,
      scope,
      state,
      slots: Vec::new(),
    };
    let props: Vec<(Prop, KeptValue)> = self
      .entries()
      .filter(|(prop, _)| include(prop.group()))
      .filter_map(|(prop, declared)| Some((*prop, declared.compile(&mut cx)?)))
      .collect();
    let transitions: Vec<Transition> = self
      .transitions()
      .iter()
      .filter(|transition| include(transition.prop.group()))
      .copied()
      .collect();
    let mut text = String::new();

    let _ = write!(text, "{props:?}{transitions:?}");

    CompiledStyle {
      address: fnv1a(&text),
      props,
      transitions,
      slots: cx.slots,
    }
  }

  /// The run properties resolved whole, as a run of rich text carries them
  /// ([`crate::seam::content::Run`]), subscribing the running effect to what it read.
  #[must_use]
  pub fn resolve_run(&self, scope: &ScopeChain) -> Rc<[(Prop, Resolved)]> {
    self
      .compile(Keep::Nothing, |group| group == Group::Run, scope, &|_| {
        false
      })
      .props
      .into_iter()
      .filter_map(|(prop, value)| match value.base {
        KeptLeaf::Value(resolved) => Some((prop, resolved)),
        _ => None,
      })
      .collect()
  }

  /// The states the record's values vary under, for the groups `include` accepts: what a
  /// realization reports for a node where it does not keep them.
  #[must_use]
  pub fn states(&self, include: impl Fn(Group) -> bool) -> StateSet {
    let states = Cell::new(StateSet::EMPTY);

    for (prop, declared) in self.entries() {
      if !include(prop.group()) {
        continue;
      }

      let scope = ScopeChain::root();
      let mut cx = CompileCx {
        keep: Keep::Everything,
        scope: &scope,
        state: &|_| false,
        slots: Vec::new(),
      };

      crate::untrack(|| {
        if let Some(kept) = declared.compile(&mut cx) {
          for (condition, _) in &kept.variants {
            if let KeptCondition::State(state) = condition {
              states.set(states.get().with(*state));
            }
          }
        }
      });
    }

    states.get()
  }
}

/// A token's definition as a realization keeps it: [`crate::seam::Realization::define_tokens`]'s
/// input.
#[derive(Clone, PartialEq, Debug)]
pub struct KeptDefinition {
  /// The token's name.
  pub name: &'static str,
  /// Its definition's content tag where [`Keep::tags_tokens`] is (`0` elsewhere): the name
  /// a node's kept leaf refers to is the pair.
  pub tag: u64,
  /// Its value, conditions kept where [`Keep::media`] is.
  pub value: KeptValue,
}

/// A scope's definitions compiled to `keep`: media variants kept or answered now, a
/// definition naming another token kept by name where `keep` keeps every token, resolved
/// otherwise.
#[must_use]
pub fn compile_definitions(
  definitions: &Definitions,
  keep: Keep,
  scope: &ScopeChain,
) -> Vec<KeptDefinition> {
  let environment = Environment::current();
  let tokens = scope.tokens();
  let leaf = |leaf: &DefinitionLeaf| -> Option<KeptLeaf> {
    match leaf {
      DefinitionLeaf::Value(value) => Some(KeptLeaf::Value(token_value(*value))),
      DefinitionLeaf::Token(name) if keep == Keep::Everything => scope
        .defines(name)
        .then_some(KeptLeaf::Token { name, tag: 0 }),
      DefinitionLeaf::Token(name) => tokens
        .resolve(name)
        .map(|value| KeptLeaf::Value(token_value(value))),
    }
  };
  let definition = |definition: &Definition| -> Option<KeptValue> {
    let mut kept = KeptValue {
      base: leaf(&definition.base)?,
      variants: Vec::new(),
    };

    for (media, variant) in &definition.variants {
      let Some(variant) = leaf(variant) else {
        continue;
      };

      if keep.media() {
        kept.variants.push((KeptCondition::Media(*media), variant));
      } else if environment.holds(media) {
        kept = KeptValue {
          base: variant,
          variants: Vec::new(),
        };
      }
    }

    Some(kept)
  };

  definitions
    .iter()
    .filter_map(|(name, value)| {
      Some(KeptDefinition {
        name,
        tag: if keep.tags_tokens() {
          scope.tag(name)?
        } else {
          0
        },
        value: definition(value)?,
      })
    })
    .collect()
}

const fn token_value(value: TokenValue) -> Resolved {
  match value {
    TokenValue::Color(color) => Resolved::Color(color),
    TokenValue::Number(number) => Resolved::Number(number),
  }
}

/// A wide FNV-1a hash of `text`: the content address every realization derives class names
/// from, identical on every side of the web.
#[must_use]
pub fn fnv1a(text: &str) -> u64 {
  text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
    (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
  })
}
