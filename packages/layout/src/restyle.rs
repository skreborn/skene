//! What moved since a platform last restyled its tree, and so which nodes it revisits
//! next.

use rustc_hash::FxHashSet;
use skene_foundation::NodeId;

/// What moved since the last restyle, kept between restyles.
///
/// The next restyle revisits every node once the definitions, the environment or the
/// structure moved; otherwise the nodes queued since the last one, those still in
/// motion at it and, where the scene's interaction moved since, those whose last
/// resolution read it.
#[derive(Debug)]
pub struct Restyle {
  generation: u64,
  every: bool,
  structure: bool,
  queued: Vec<NodeId>,
  moving: Vec<NodeId>,
  interactive: FxHashSet<NodeId>,
  interaction: u64,
}

/// The nodes a restyle revisits.
#[derive(Debug, PartialEq, Eq)]
pub enum Revisit {
  /// Every node.
  Every,
  /// The nodes queued and in motion, and whether the scene's interaction moved, so the
  /// nodes whose resolution read it ([`Restyle::interactive`]) are revisited too where
  /// theirs did.
  Nodes(Vec<NodeId>, bool),
}

impl Default for Restyle {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl Restyle {
  /// What moved before a first restyle: everything, the resolution generation at one,
  /// so a resolution stamped zero is always stale.
  #[must_use]
  pub fn new() -> Self {
    Self {
      generation: 1,
      every: true,
      structure: true,
      queued: Vec::new(),
      moving: Vec::new(),
      interactive: FxHashSet::default(),
      interaction: 0,
    }
  }

  /// The resolution generation, moving wherever every resolved style goes stale.
  #[inline(always)]
  #[must_use]
  pub const fn generation(&self) -> u64 {
    self.generation
  }

  /// Marks every resolved style stale: the definitions or the environment moved, and the
  /// next restyle revisits every node.
  #[inline(always)]
  pub const fn invalidate(&mut self) {
    self.generation += 1;
    self.every = true;
  }

  /// Revisits every node at the next restyle, keeping the resolved styles.
  #[inline(always)]
  pub const fn revisit_every(&mut self) {
    self.every = true;
  }

  /// Notes that the structure moved: the next restyle revisits every node, and
  /// [`take_structure`](Self::take_structure) answers `true` once.
  #[inline(always)]
  pub const fn move_structure(&mut self) {
    self.structure = true;
  }

  /// Whether the structure moved since this last answered.
  #[inline(always)]
  pub const fn take_structure(&mut self) -> bool {
    std::mem::replace(&mut self.structure, false)
  }

  /// Queues `node` for the next restyle: a declaration, a text, a value or a measure of
  /// its moved.
  #[inline(always)]
  pub fn queue(&mut self, node: NodeId) {
    self.queued.push(node);
  }

  /// Revisits `node` at the next restyle, its motion still running.
  #[inline(always)]
  pub fn keep_moving(&mut self, node: NodeId) {
    self.moving.push(node);
  }

  /// Notes whether `node`'s last resolution read its interaction.
  #[inline(always)]
  pub fn read_interaction(&mut self, node: NodeId, read: bool) {
    if read {
      self.interactive.insert(node);
    }
    else {
      self.interactive.remove(&node);
    }
  }

  /// The nodes whose last resolution read their interaction.
  #[inline(always)]
  pub fn interactive(&self) -> impl Iterator<Item = NodeId> + '_ {
    self.interactive.iter().copied()
  }

  /// Whether the next restyle revisits any node, the scene's interaction at
  /// `interaction`.
  #[inline(always)]
  #[must_use]
  pub const fn is_pending(&self, interaction: u64) -> bool {
    self.every || self.structure || !self.queued.is_empty() || self.interaction != interaction
  }

  /// Takes the nodes the next restyle revisits, the scene's interaction now at
  /// `interaction`.
  pub fn take(&mut self, interaction: u64) -> Revisit {
    let moved = std::mem::replace(&mut self.interaction, interaction) != interaction;
    let queued = std::mem::take(&mut self.queued);
    let moving = std::mem::take(&mut self.moving);

    if std::mem::replace(&mut self.every, false) || self.structure {
      return Revisit::Every;
    }

    let mut nodes = queued;

    nodes.extend(moving);

    Revisit::Nodes(nodes, moved)
  }
}
