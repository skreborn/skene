//! What a drawn platform's realization forwards to: creating and destroying a node's
//! record, applying its style, slots, state and contents, observing its size, opening an
//! overlay with the focus move it asks for, and mounting a view under the root.

use std::rc::Rc;

use skene_foundation::{
  ContentValue,
  NativeElement,
  NodeFacts,
  NodeId,
  NodeSpec,
  Observation,
  Placement,
  Realization,
  RunPlacement,
  Scene,
  Slot,
  reactive::Owner,
};

use super::{BoxRecord, Drawn, KeptStyle, Kind, Observer, Record, Runtime, SlotLeaf};

/// The character standing in for an inline box in its paragraph's text: a run laid out
/// as a box flows as this one placeholder cluster, as large as its own layout.
pub const INLINE_BOX: char = '\u{fffc}';

/// The focus move an overlay's opening or closing asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum OverlayFocus {
  /// Focus stays where it is.
  Held,
  /// Focus moves to the node.
  Moved(NodeId),
}

impl<P: Drawn> Runtime<P> {
  /// Creates the record of `node` from `spec`, carrying the platform's own state of it
  /// ([`Drawn::node`]), joining the overlays or the text entries where it is one, an
  /// inline box's text its placeholder ([`INLINE_BOX`]); `false` where `spec` realizes
  /// no node of its own (an anchor or a scope), which keeps none.
  pub fn create(&mut self, node: NodeId, spec: &NodeSpec) -> bool {
    let inner = |boxed| BoxRecord::new(spec.role, spec.drag, boxed);
    let kind = match &spec.facts {
      NodeFacts::Box(boxed) => Kind::Box(inner(boxed.clone())),
      NodeFacts::Control(control) => {
        if control.native.is_some_and(NativeElement::is_text_entry) {
          self.entries.push(node);
        }

        Kind::Control {
          native: control.native,
          label: control.label.clone(),
          inner: inner(control.boxed.clone()),
        }
      }
      NodeFacts::Overlay(overlay) => {
        self.overlays.push(node);

        Kind::Overlay {
          inner: inner(overlay.boxed.clone()),
          placement: Placement::Center,
          backdrop: overlay.backdrop,
          open: false,
          position: None,
        }
      }
      NodeFacts::Run(run) => Kind::run(run.clone()),
      NodeFacts::Anchor | NodeFacts::Scope => return false,
    };
    let mut record = Record::new(kind, P::node(spec));

    if let NodeFacts::Run(run) = &spec.facts
      && run.placement == RunPlacement::Embedded
    {
      record.text = INLINE_BOX.to_string();
    }

    self.insert(node, record);

    true
  }

  /// Keeps `style` as the style of `node`, resolved again at the next restyle.
  pub fn apply_style(&mut self, node: NodeId, style: KeptStyle<P>) {
    if let Some(kept) = self
      .record_mut(node)
      .and_then(|record| record.kind.kept_mut())
    {
      *kept = style;
      self.restyle.queue(node);
    }
  }

  /// Keeps `leaf` as what a derived leaf last wrote into the `slot` of `node`, resolved
  /// again at the next restyle, laid out and repainted.
  pub fn apply_slot(&mut self, node: NodeId, slot: Slot, leaf: &SlotLeaf<P>) {
    if let Some(record) = self.record_mut(node) {
      record.slots.insert(slot, leaf.clone());
      record.kind.mark_stale();
      self.restyle.queue(node);
      self.layout_dirty = true;
      self.damage(node);
    }
  }

  /// Notes that the state of the control `node` moved: it is laid out and repainted, its
  /// style reading the state the scene keeps.
  pub fn set_control_state(&mut self, node: NodeId) {
    self.layout_dirty = true;
    self.damage(node);
  }

  /// Writes `value` into the record of `node`, returning whether a run's text changed, so
  /// the platform drops what it keeps of the old text.
  ///
  /// A run's text is restyled, laid out and repainted; a control's value is laid out and
  /// repainted; a link's destination and an overlay's placement are kept. An entry's
  /// edit, placeholder, secure flag and keyboard, and a door's value, are the platform's
  /// own, and leave the record as it was.
  pub fn write(&mut self, node: NodeId, value: ContentValue<'_>) -> bool {
    match value {
      ContentValue::Text(text) => {
        let Some(record) = self.record_mut(node)
        else {
          return false;
        };

        if record.text == text {
          return false;
        }

        text.clone_into(&mut record.text);
        self.restyle.queue(node);
        self.layout_dirty = true;
        self.damage(node);

        return true;
      }
      ContentValue::Checked(_)
      | ContentValue::Range(_)
      | ContentValue::Choice(_)
      | ContentValue::Chosen(_)
      | ContentValue::Membership(_) => {
        self.layout_dirty = true;
        self.damage(node);
      }
      ContentValue::Destination(path) => {
        if let Some(Kind::Run { destination, .. }) =
          self.record_mut(node).map(|record| &mut record.kind)
        {
          *destination = Some(path.to_owned());
          self.damage(node);
        }
      }
      ContentValue::Placement(placement) => {
        if let Some(Kind::Overlay {
          placement: held,
          position,
          ..
        }) = self.record_mut(node).map(|record| &mut record.kind)
        {
          *held = placement.placement;
          *position = placement.position;
          self.paint_dirty = true;
          self.move_geometry();
        }
      }
      ContentValue::Edit(_)
      | ContentValue::Placeholder(_)
      | ContentValue::Secure(_)
      | ContentValue::Keyboard(_)
      | ContentValue::Door { .. } => {}
    }

    false
  }

  /// Observes the size of the box `node`: `report` hears the size each layout gives it
  /// that differs from the last it heard, the first layout after now included, until the
  /// observation ends.
  ///
  /// # Panics
  ///
  /// Panics if no runtime is installed for this thread, or if the platform's state is
  /// borrowed already; the observation's end panics alike.
  pub fn observe_size(node: NodeId, report: Observer<P>) -> Observation {
    let kept = Rc::clone(&report);

    Self::with(|runtime| {
      if let Some(record) = runtime.record_mut(node) {
        record.observers.push(report);
        record.reported = None;
        runtime.layout_dirty = true;
      }
    });

    Observation::new(move || {
      Self::with(|runtime| {
        if let Some(record) = runtime.record_mut(node) {
          record
            .observers
            .retain(|observer| !Rc::ptr_eq(observer, &kept));
        }
      });
    })
  }

  /// Opens or closes the overlay `node`, returning the focus move the change asks for: a
  /// modal opening takes focus as the scene chooses
  /// ([`Scene::modal_focus`](skene_foundation::Scene::modal_focus)), and the scene
  /// returns focus as an overlay closes. Every node is resolved again where the overlay
  /// opened or closed, and the frame is laid out and repainted.
  pub fn set_overlay_open(&mut self, node: NodeId, open: bool) -> OverlayFocus {
    let Some(Kind::Overlay { open: held, .. }) =
      self.record_mut(node).map(|record| &mut record.kind)
    else {
      return OverlayFocus::Held;
    };
    let was_open = std::mem::replace(held, open);

    if was_open != open {
      self.restyle.revisit_every();
    }

    self.layout_dirty = true;
    self.paint_dirty = true;
    self.move_geometry();

    if !open || was_open || !Scene::<P>::read(|scene| scene.is_modal(node)) {
      return OverlayFocus::Held;
    }

    let order = self.tab_order();

    OverlayFocus::Moved(Scene::<P>::read(|scene| scene.modal_focus(node, &order)))
  }

  /// Every open overlay reachable from the root, in the order they opened; later
  /// entries stack on top.
  #[must_use]
  pub fn open_overlays(&self) -> Vec<NodeId> {
    Scene::<P>::read(|scene| scene.open_overlays().to_vec())
      .into_iter()
      .filter(|overlay| self.is_reachable(*overlay))
      .collect()
  }

  /// The tab stops in traversal order, as the scene orders them: document order, the
  /// scene's child order; closed overlays and invisible boxes excluded, and an open modal
  /// containing the whole order with the overlays opened after it.
  #[must_use]
  pub fn tab_order(&self) -> Vec<NodeId> {
    let overlays: Vec<NodeId> = self
      .open_overlays()
      .into_iter()
      .filter(|&overlay| self.contains(overlay))
      .collect();

    Scene::<P>::read(|scene| {
      scene.tab_order(self.root, &overlays, |node| {
        scene.contains(node) && self.is_invisible(node)
      })
    })
  }

  /// Whether the box of `node` is invisible by its style: it and its content draw
  /// nothing and take neither the pointer nor focus.
  #[must_use]
  pub fn is_invisible(&self, node: NodeId) -> bool {
    self.record(node).is_some_and(Record::is_invisible)
  }

  /// Builds `view` under the root as one turn, returning its node.
  ///
  /// The first mount opens the window: once its turn ends, the control set to autofocus
  /// that comes first in the scene's child order takes focus through the platform,
  /// without showing it; nothing is focused where no control asked.
  ///
  /// # Panics
  ///
  /// Panics if no runtime is installed for this thread, or if the platform's state is
  /// borrowed already.
  pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
    skene_foundation::end_turn();

    let root = Self::with(|runtime| runtime.root);
    let node = Owner::root().enter(|| {
      let node = view();

      Scene::<P>::with(|scene| scene.attach(root, node, None));

      node
    });

    skene_foundation::end_turn();

    let opening = Self::with(|runtime| {
      Scene::<P>::with(|scene| scene.take_autofocus(root, |node| runtime.is_invisible(node)))
    });

    if let Some(opening) = opening {
      <P as Realization>::focus(opening);
      skene_foundation::end_turn();
    }

    node
  }
}
