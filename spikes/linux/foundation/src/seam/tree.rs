//! The shared half: the scene of realized nodes and everything that calls the realization.

use std::{
  any::{Any, TypeId},
  cell::RefCell,
  collections::HashMap,
  rc::Rc,
};

use crate::{
  Definitions, Effect, NodeId, Owner, Scene, Signal, State, StateSet,
  layout::ResolvedLayout,
  seam::{
    LayoutOwner, Realization,
    access::AccessFacts,
    activity,
    content::{Content, ContentKey, ContentType, Controller, PropValue, Verdict, writing},
    door::ControlSink,
    node::{NodeKind, NodeSpec},
    record::{Group, Record},
    report::{Answer, Interest, KeyEvent, PointerEvent, Report, Size},
    style::{CompiledStyle, Keep, ScopeChain, compile_definitions},
  },
  untrack,
};

/// What a box's event handler answers: whether the event goes on to the ancestors.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Bubbling {
  /// It goes on.
  #[default]
  Continue,
  /// It stops here.
  Stop,
}

impl From<()> for Bubbling {
  #[inline(always)]
  fn from((): ()) -> Self {
    Self::Continue
  }
}

type Handler<E> = Rc<dyn Fn(&E) -> Bubbling>;

#[derive(Default)]
struct Handlers {
  activate: Option<Rc<dyn Fn()>>,
  pointer: Option<Handler<PointerEvent>>,
  key: Option<Handler<KeyEvent>>,
  follow: Option<Handler<str>>,
  focus: Option<Rc<dyn Fn(bool)>>,
  size: Option<Rc<dyn Fn(Size)>>,
}

struct Entry<R: Realization> {
  spec: NodeSpec,
  native: Option<R::Node>,
  scope: ScopeChain,
  states: Signal<StateSet>,
  record: Option<Rc<Record>>,
  controls: HashMap<Content, Rc<Controller<R>>>,
  sink: Option<Rc<dyn ControlSink>>,
  handlers: Handlers,
  interest: Interest,
  access: Vec<Rc<dyn Fn() -> AccessFacts>>,
  access_version: Option<Signal<u32>>,
}

struct Tree<R: Realization> {
  scene: Scene<Entry<R>>,
}

thread_local! {
  static TREES: RefCell<HashMap<TypeId, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

fn with<R: Realization, T>(body: impl FnOnce(&mut Tree<R>) -> T) -> T {
  TREES.with(|trees| {
    let mut trees = trees.borrow_mut();
    let tree = trees
      .entry(TypeId::of::<R>())
      .or_insert_with(|| {
        Box::new(Tree::<R> {
          scene: Scene::new(),
        })
      })
      .downcast_mut::<Tree<R>>()
      .expect("one tree per realization");

    body(tree)
  })
}

/// The realized node of `id`, where it has one.
#[must_use]
pub fn native<R: Realization>(id: NodeId) -> Option<R::Node> {
  with::<R, _>(|tree| tree.scene.get(id).and_then(|entry| entry.native.clone()))
}

/// The node's spec.
#[must_use]
pub fn spec<R: Realization>(id: NodeId) -> Option<NodeSpec> {
  with::<R, _>(|tree| tree.scene.get(id).map(|entry| entry.spec))
}

/// The node's record, for the harness's outlines; never the accessibility truth.
#[must_use]
pub fn record<R: Realization>(id: NodeId) -> Option<Rc<Record>> {
  with::<R, _>(|tree| tree.scene.get(id).and_then(|entry| entry.record.clone()))
}

/// The node's parent in the scene.
#[must_use]
pub fn parent<R: Realization>(id: NodeId) -> Option<NodeId> {
  with::<R, _>(|tree| tree.scene.parent(id))
}

/// The node's children in the scene.
#[must_use]
pub fn children<R: Realization>(id: NodeId) -> Vec<NodeId> {
  with::<R, _>(|tree| tree.scene.children(id).to_vec())
}

fn insert_entry<R: Realization>(spec: NodeSpec, scope: &ScopeChain) -> NodeId {
  with::<R, _>(|tree| {
    tree.scene.insert(Entry {
      spec,
      native: None,
      scope: scope.clone(),
      states: Signal::new(StateSet::EMPTY),
      record: None,
      controls: HashMap::new(),
      sink: None,
      handlers: Handlers::default(),
      interest: Interest::default(),
      access: Vec::new(),
      access_version: None,
    })
  })
}

fn set_native<R: Realization>(id: NodeId, native: R::Node) {
  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.native = Some(native);
    }
  });
}

/// Whether a node of `kind` is realized: every kind but a segment, and a scope only where the
/// realization realizes scopes.
#[must_use]
pub const fn realizes<R: Realization>(kind: NodeKind) -> bool {
  match kind {
    NodeKind::Segment => false,
    NodeKind::Scope => R::SCOPE_NODES,
    _ => true,
  }
}

/// Creates a node of `spec` in `scope`, realized at once where its kind is ([`realizes`]),
/// forgotten when the current owner is disposed.
pub fn create<R: Realization>(spec: NodeSpec, scope: &ScopeChain) -> NodeId {
  let id = insert_entry::<R>(spec, scope);

  if realizes::<R>(spec.kind) {
    set_native::<R>(id, R::create(id, &spec));
  }

  forget_on_cleanup::<R>(id);

  id
}

/// Adopts a toolkit widget through the native door. Where the widget holds values, `sink`
/// reads and writes them, and [`control`] makes them controlled exactly as a built-in's.
pub fn adopt<R: Realization>(
  widget: R::Widget,
  spec: NodeSpec,
  scope: &ScopeChain,
  sink: Option<Rc<dyn ControlSink>>,
) -> NodeId {
  let id = insert_entry::<R>(spec, scope);
  let native = R::adopt(id, widget, &spec);

  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.native = Some(native);
      entry.sink = sink;
    }
  });

  forget_on_cleanup::<R>(id);

  id
}

/// Forgets the node's handlers and controllers when the current owner is disposed, and
/// removes its realized node for good ([`Realization::remove`]), so what it held (its style's
/// reference) is released.
fn forget_on_cleanup<R: Realization>(id: NodeId) {
  if Owner::current().is_some() {
    Owner::on_cleanup(move || {
      let native = with::<R, _>(|tree| {
        let entry = tree.scene.get_mut(id)?;

        entry.controls.clear();
        entry.handlers = Handlers::default();
        entry.access.clear();
        entry.native.take()
      });

      if let Some(native) = native {
        R::remove(&native);
      }
    });
  }
}

/// The realized node `id`'s children join, and the index among them its children start at:
/// the nearest realized ancestor, past unrealized scopes.
fn realized_place<R: Realization>(tree: &Tree<R>, id: NodeId) -> Option<(NodeId, usize)> {
  let parent = tree.scene.parent(id)?;
  let before: usize = tree
    .scene
    .children(parent)
    .iter()
    .take_while(|child| **child != id)
    .map(|child| realized_count(tree, *child))
    .sum();

  if tree.scene.get(parent)?.native.is_some() {
    return Some((parent, before));
  }

  let (ancestor, start) = realized_place(tree, parent)?;

  Some((ancestor, start + before))
}

fn realized_count<R: Realization>(tree: &Tree<R>, id: NodeId) -> usize {
  match tree.scene.get(id) {
    Some(entry) if entry.native.is_some() => 1,
    Some(_) => tree
      .scene
      .children(id)
      .iter()
      .map(|child| realized_count(tree, *child))
      .sum(),
    None => 0,
  }
}

fn realized_roots<R: Realization>(tree: &Tree<R>, id: NodeId, out: &mut Vec<R::Node>) {
  if let Some(entry) = tree.scene.get(id) {
    match &entry.native {
      Some(native) => out.push(native.clone()),
      None => {
        for child in tree.scene.children(id) {
          realized_roots(tree, *child, out);
        }
      }
    }
  }
}

/// Attaches `child` under `parent` at `index` among the scene's children, placing its
/// realized nodes; a child already under `parent` is relocated, keeping what it holds.
pub fn attach<R: Realization>(parent: NodeId, child: NodeId, index: usize) {
  let moving = with::<R, _>(|tree| tree.scene.parent(child) == Some(parent));
  let placed = with::<R, _>(|tree| {
    tree.scene.attach(parent, child, index);

    let (target, start) = realized_place(tree, child)?;
    let target = tree.scene.get(target)?.native.clone()?;
    let mut roots = Vec::new();

    realized_roots(tree, child, &mut roots);

    Some((target, start, roots))
  });

  if let Some((target, start, roots)) = placed {
    for (offset, root) in roots.iter().enumerate() {
      if moving {
        R::relocate(&target, root, start + offset);
      } else {
        R::insert(&target, root, start + offset);
      }
    }
  }
}

/// Hides a node to show it again later, or shows it.
pub fn set_hidden<R: Realization>(id: NodeId, hidden: bool) {
  let roots = with::<R, _>(|tree| {
    let mut roots = Vec::new();

    realized_roots(tree, id, &mut roots);

    roots
  });

  for root in roots {
    R::set_hidden(&root, hidden);
  }
}

/// Removes a node and its subtree for good.
pub fn remove<R: Realization>(id: NodeId) {
  let roots = with::<R, _>(|tree| {
    let mut roots = Vec::new();

    realized_roots(tree, id, &mut roots);

    roots
  });

  for root in roots {
    R::remove(&root);
  }

  with::<R, _>(|tree| tree.scene.remove(id));
}

/// Gives a node its record: one effect keeps its compiled style on the realization and, on
/// [`LayoutOwner::Engine`] platforms, one keeps its resolved layout on its host container.
pub fn set_record<R: Realization>(id: NodeId, record: Record) {
  let record = Rc::new(record);
  let Some((native, scope, states)) = with::<R, _>(|tree| {
    let entry = tree.scene.get_mut(id)?;

    entry.record = Some(Rc::clone(&record));

    Some((entry.native.clone()?, entry.scope.clone(), entry.states))
  }) else {
    return;
  };
  let engine = R::LAYOUT == LayoutOwner::Engine;
  let styled = move |group: Group| !engine || matches!(group, Group::Box | Group::Run);
  let laid_out = move |group: Group| matches!(group, Group::Layout | Group::Placement);
  let mut watched = record.states(laid_out);

  if !R::KEEP.states() {
    watched = watched.union(record.states(styled));
  }

  if !watched.is_empty() {
    let interest = with::<R, _>(|tree| {
      let entry = tree.scene.get_mut(id)?;

      entry.interest.states = entry.interest.states.union(watched);

      Some(entry.interest)
    });

    if let Some(interest) = interest {
      R::watch(&native, interest);
    }
  }

  {
    let (record, native, scope) = (Rc::clone(&record), native.clone(), scope.clone());
    let last = RefCell::new(None::<CompiledStyle>);

    Effect::new(move || {
      let holding = states.get();
      let compiled = record.compile(R::KEEP, styled, &scope, &|state| holding.contains(state));
      let mut last = last.borrow_mut();

      match last.as_ref() {
        Some(previous) if previous.address == compiled.address => {
          if previous.slots != compiled.slots {
            untrack(|| R::apply_slots(&native, &compiled.slots));
          }
        }
        None if compiled.is_empty() => {}
        _ => untrack(|| R::apply_style(&native, &compiled)),
      }

      *last = Some(compiled);
    });
  }

  if engine {
    let last = RefCell::new(None::<ResolvedLayout>);

    Effect::new(move || {
      let holding = states.get();
      let compiled = record.compile(Keep::Nothing, laid_out, &scope, &|state| {
        holding.contains(state)
      });
      let layout = ResolvedLayout::from_compiled(&compiled);
      let mut last = last.borrow_mut();

      if last.as_ref() != Some(&layout) {
        untrack(|| R::apply_layout(&native, &layout));
        *last = Some(layout);
      }
    });
  }
}

fn check_takes<R: Realization>(id: NodeId, content: Content) {
  if let Some(spec) = spec::<R>(id) {
    assert!(
      spec.kind.takes(content),
      "a {:?} takes no {content:?} (it takes {:?})",
      spec.kind,
      spec.kind.contents()
    );
  }
}

/// What the platform shows of `content` at `id`: through the door's sink where a library's
/// widget holds it, through the realization elsewhere.
pub(crate) fn read_content<R: Realization>(id: NodeId, content: Content) -> Option<PropValue> {
  let (native, sink) = with::<R, _>(|tree| {
    let entry = tree.scene.get(id)?;

    Some((entry.native.clone(), entry.sink.clone()))
  })?;

  match sink {
    Some(sink) => sink.read(content),
    None => R::read(&native?, content),
  }
}

/// Shows `value` as `content` at `id`, through the door's sink or the realization.
pub(crate) fn write_content<R: Realization>(id: NodeId, content: Content, value: &PropValue) {
  let Some((native, sink)) = with::<R, _>(|tree| {
    let entry = tree.scene.get(id)?;

    Some((entry.native.clone(), entry.sink.clone()))
  }) else {
    return;
  };

  match (sink, native) {
    (Some(sink), _) => sink.write(content, value),
    (None, Some(native)) => R::write(&native, content, value),
    (None, None) => {}
  }
}

/// Whether an input method composes at `id`.
pub(crate) fn composing<R: Realization>(id: NodeId) -> bool {
  let Some((native, sink)) = with::<R, _>(|tree| {
    let entry = tree.scene.get(id)?;

    Some((entry.native.clone(), entry.sink.clone()))
  }) else {
    return false;
  };

  match (sink, native) {
    (Some(sink), _) => sink.composing(),
    (None, Some(native)) => R::composing(&native),
    (None, None) => false,
  }
}

/// Keeps a content value at what `derive` returns, uncontrolled: a text's content, a
/// placeholder, a caption, a painted node's pixels. The reader cannot change it, so a value
/// equal to the last one written is not written again, and nothing is read back.
///
/// # Panics
///
/// Where the node's kind does not take `key`'s content ([`NodeKind::contents`]).
pub fn bind<R: Realization, T: ContentType>(
  id: NodeId,
  key: ContentKey<T>,
  derive: impl Fn() -> T + 'static,
) {
  let content = key.content();
  let last = RefCell::new(None::<PropValue>);

  check_takes::<R>(id, content);
  Effect::new(move || {
    let value = derive().into_value();
    let mut last = last.borrow_mut();

    if last.as_ref() != Some(&value) {
      untrack(|| writing(|| write_content::<R>(id, content, &value)));
      *last = Some(value);
    }
  });
}

/// Makes a content value controlled: the platform shows what `derive` returns, a change the
/// reader makes is reported to `handler` and answered with the truth.
///
/// # Panics
///
/// Where the node's kind does not take `key`'s content ([`NodeKind::contents`]).
pub fn control<R: Realization, T: ContentType>(
  id: NodeId,
  key: ContentKey<T>,
  derive: impl Fn() -> T + 'static,
  handler: Option<Box<dyn Fn(T)>>,
) -> Rc<Controller<R>> {
  let content = key.content();

  check_takes::<R>(id, content);

  let handler = handler.map(|handler| {
    Box::new(move |value: PropValue| {
      if let Some(value) = T::from_value(&value) {
        handler(value);
      }
    }) as Box<dyn Fn(PropValue)>
  });
  let controller = Controller::<R>::new(
    id,
    content,
    Box::new(move || derive().into_value()),
    handler,
  );

  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.controls.insert(content, Rc::clone(&controller));
    }
  });

  let shown = Rc::clone(&controller);

  Effect::new(move || {
    let truth = shown.tracked_truth();

    untrack(|| shown.show(&truth));
  });

  controller
}

/// The controller of `content` on `id`.
#[must_use]
pub fn controller<R: Realization>(id: NodeId, content: Content) -> Option<Rc<Controller<R>>> {
  with::<R, _>(|tree| tree.scene.get(id)?.controls.get(&content).cloned())
}

fn watch_now<R: Realization>(id: NodeId, change: impl FnOnce(&mut Entry<R>)) {
  let watched = with::<R, _>(|tree| {
    let entry = tree.scene.get_mut(id)?;

    change(entry);

    Some((entry.native.clone()?, entry.interest))
  });

  if let Some((native, interest)) = watched {
    R::watch(&native, interest);
  }
}

/// Runs `activate` when the node is activated.
pub fn handle_activate<R: Realization>(id: NodeId, activate: impl Fn() + 'static) {
  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.handlers.activate = Some(Rc::new(activate));
    }
  });
}

/// Runs `follow` with each destination followed at the node or beneath it.
pub fn handle_follow<R: Realization, B: Into<Bubbling>>(
  id: NodeId,
  follow: impl Fn(&str) -> B + 'static,
) {
  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.handlers.follow = Some(Rc::new(move |destination: &str| follow(destination).into()));
    }
  });
}

/// Runs `pointer` with the pointer events at the node or beneath it.
pub fn handle_pointer<R: Realization, B: Into<Bubbling>>(
  id: NodeId,
  pointer: impl Fn(&PointerEvent) -> B + 'static,
) {
  watch_now::<R>(id, |entry| {
    entry.handlers.pointer = Some(Rc::new(move |event| pointer(event).into()));
    entry.interest.pointer = true;
  });
}

/// Runs `key` with the key events while focus is on the node or beneath it, those the
/// toolkit's controls did not consume.
pub fn handle_key<R: Realization, B: Into<Bubbling>>(
  id: NodeId,
  key: impl Fn(&KeyEvent) -> B + 'static,
) {
  watch_now::<R>(id, |entry| {
    entry.handlers.key = Some(Rc::new(move |event| key(event).into()));
    entry.interest.keys = true;
  });
}

/// Runs `focus` when focus enters or leaves the node.
pub fn handle_focus<R: Realization>(id: NodeId, focus: impl Fn(bool) + 'static) {
  watch_now::<R>(id, |entry| {
    entry.handlers.focus = Some(Rc::new(focus));
    entry.interest.focus = true;
  });
}

/// Runs `size` with the node's size each time it is laid out at another.
pub fn handle_size<R: Realization>(id: NodeId, size: impl Fn(Size) + 'static) {
  watch_now::<R>(id, |entry| {
    entry.handlers.size = Some(Rc::new(size));
    entry.interest.size = true;
  });
}

/// Adds a view's contribution to the node's accessibility facts, kept current: one effect
/// per node merges every contribution ([`AccessFacts::merge`]) and hands the realization
/// what changed.
pub fn add_access<R: Realization>(id: NodeId, derive: impl Fn() -> AccessFacts + 'static) {
  let Some((version, first)) = with::<R, _>(|tree| {
    let entry = tree.scene.get_mut(id)?;
    let first = entry.access_version.is_none();

    entry.access.push(Rc::new(derive));

    Some((
      *entry.access_version.get_or_insert_with(|| Signal::new(0)),
      first,
    ))
  }) else {
    return;
  };

  if !first {
    version.update(|version| *version += 1);

    return;
  }

  let previous = RefCell::new(AccessFacts::default());

  Effect::new(move || {
    let _ = version.get();
    let sources = with::<R, _>(|tree| {
      tree
        .scene
        .get(id)
        .map(|entry| entry.access.clone())
        .unwrap_or_default()
    });
    let mut facts = AccessFacts::default();

    for source in sources {
      facts.merge(source());
    }

    let mut previous = previous.borrow_mut();

    if *previous != facts {
      if let Some(node) = native::<R>(id) {
        untrack(|| R::apply_access(&node, &previous, &facts, &|related| native::<R>(related)));
      }

      *previous = facts;
    }
  });
}

/// A theme scope inside `parent`, its definitions kept on the realization by one effect and
/// withdrawn from it when the current owner is disposed.
pub fn scope<R: Realization>(
  parent: &ScopeChain,
  definitions: Signal<Rc<Definitions>>,
  node: Option<NodeId>,
) -> ScopeChain {
  let chain = parent.scope(definitions);
  let id = chain.id().expect("a scope");
  let effect_chain = chain.clone();

  Effect::new(move || {
    let compiled =
      definitions.with(|definitions| compile_definitions(definitions, R::KEEP, &effect_chain));
    let native = node.and_then(native::<R>);

    untrack(|| R::define_tokens(id, native.as_ref(), &compiled));
  });

  if Owner::current().is_some() {
    Owner::on_cleanup(move || {
      let native = node.and_then(native::<R>);

      R::define_tokens(id, native.as_ref(), &[]);
    });
  }

  chain
}

/// What a realization calls when the reader acted at `id`: runs the handlers and answers.
///
/// A change or an edit is answered with its verdict at once, so a toolkit that asks before
/// applying is told what to apply instead and one that applied first what to repair to. A
/// pointer event climbs the scene from `id`, deepest first, and stops at a control or an
/// acting box (a view that handles an interaction owns it) or at a handler that stops it; a
/// key event or a followed destination climbs until a handler stops it.
pub fn report<R: Realization>(id: NodeId, report: Report) -> Answer {
  activity::report();

  match report {
    Report::Change {
      content,
      proposed,
      applied,
    } => Answer::Verdict(
      controller::<R>(id, content).map_or(Verdict::Ignore, |controller| {
        controller.report(proposed, applied)
      }),
    ),
    Report::Edit {
      start,
      end,
      replacement,
      applied,
    } => Answer::Verdict(
      controller::<R>(id, Content::Edit).map_or(Verdict::Ignore, |controller| {
        controller.report_edit(start, end, &replacement, applied)
      }),
    ),
    Report::Selection { anchor, caret } => Answer::Verdict(
      controller::<R>(id, Content::Edit).map_or(Verdict::Ignore, |controller| {
        controller.report_selection(anchor, caret)
      }),
    ),
    Report::Activate | Report::Action(_) => activate::<R>(id),
    Report::Follow(destination) => {
      bubble::<R, str>(id, &destination, |handlers| handlers.follow.clone(), false)
    }
    Report::Composition(composing) => {
      if !composing {
        let controllers: Vec<Rc<Controller<R>>> = with::<R, _>(|tree| {
          tree
            .scene
            .get(id)
            .map(|entry| entry.controls.values().cloned().collect())
            .unwrap_or_default()
        });

        for controller in controllers {
          if controller.is_pending() {
            controller.settle();
          }
        }
      }

      Answer::None
    }
    Report::Focus(focused) => {
      let focus = with::<R, _>(|tree| tree.scene.get(id)?.handlers.focus.clone());

      if let Some(focus) = focus {
        untrack(|| focus(focused));
      }

      Answer::None
    }
    Report::State { state, holds } => {
      let states = with::<R, _>(|tree| tree.scene.get(id).map(|entry| entry.states));

      if let Some(states) = states {
        let current = states.get_untracked();
        let next = if holds {
          current.with(state)
        } else {
          current.without(state)
        };

        if next != current {
          states.set(next);
        }
      }

      Answer::None
    }
    Report::Pointer(event) => {
      bubble::<R, PointerEvent>(id, &event, |handlers| handlers.pointer.clone(), true)
    }
    Report::Size(size) => {
      let handler = with::<R, _>(|tree| tree.scene.get(id)?.handlers.size.clone());

      if let Some(handler) = handler {
        untrack(|| handler(size));
      }

      Answer::None
    }
    Report::Key(event) => bubble::<R, KeyEvent>(id, &event, |handlers| handlers.key.clone(), false),
  }
}

fn activate<R: Realization>(id: NodeId) -> Answer {
  let activate = with::<R, _>(|tree| tree.scene.get(id)?.handlers.activate.clone());

  if let Some(activate) = &activate {
    untrack(|| activate());
  }

  Answer::Event {
    handled: activate.is_some(),
  }
}

fn bubble<R: Realization, E: ?Sized>(
  id: NodeId,
  event: &E,
  pick: impl Fn(&Handlers) -> Option<Handler<E>>,
  stops_at_interactive: bool,
) -> Answer {
  let path = with::<R, _>(|tree| {
    std::iter::once(id)
      .chain(tree.scene.ancestors(id))
      .map(|node| {
        let entry = tree.scene.get(node);

        (
          entry.is_some_and(|entry| entry.spec.is_interactive()),
          entry.and_then(|entry| pick(&entry.handlers)),
        )
      })
      .collect::<Vec<_>>()
  });
  let mut handled = false;

  for (interactive, handler) in path {
    if let Some(handler) = handler {
      handled = true;

      if untrack(|| handler(event)) == Bubbling::Stop {
        break;
      }
    }

    if stops_at_interactive && interactive {
      break;
    }
  }

  Answer::Event { handled }
}

/// The states `id`'s style resolves against now, subscribing the running effect to them.
#[must_use]
pub fn resolved_states<R: Realization>(id: NodeId) -> StateSet {
  with::<R, _>(|tree| tree.scene.get(id).map(|entry| entry.states))
    .map_or(StateSet::EMPTY, |states| states.get())
}

/// The states a node is in, as reported.
#[must_use]
pub fn states<R: Realization>(id: NodeId) -> StateSet {
  with::<R, _>(|tree| {
    tree
      .scene
      .get(id)
      .map_or(StateSet::EMPTY, |entry| entry.states.get_untracked())
  })
}

/// Whether `state` holds at the node, as reported.
#[must_use]
pub fn holds<R: Realization>(id: NodeId, state: State) -> bool {
  states::<R>(id).contains(state)
}
