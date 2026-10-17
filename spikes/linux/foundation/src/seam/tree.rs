//! The shared half: the scene of realized nodes and everything that calls the realization.

use std::{
  any::{Any, TypeId},
  cell::RefCell,
  collections::HashMap,
  rc::Rc,
};

use crate::{
  Definitions, Effect, NodeId, Owner, Scene, Signal, State, StateSet,
  layout::{Rect, ResolvedLayout},
  seam::{
    LayoutOwner, Realization,
    access::AccessFacts,
    activity,
    content::{Content, ContentKey, ContentType, Controller, PropValue, Verdict, writing},
    control::{
      AccessRequest, Availability, ClipboardOp, ControlDispatch, ControlState, Part, Prevent,
    },
    door::ControlSink,
    node::{NodeKind, NodeSpec},
    record::{Group, Record},
    report::{
      Answer, ContextMenuEvent, DropEvent, Dropped, Interest, KeyEvent, PointerEvent, Report, Size,
    },
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

/// A drop handler: whether it took what was dropped.
type DropHandler = Rc<dyn Fn(&Dropped) -> bool>;

impl Bubbling {
  /// The verdict a box's answer gives: it keeps no platform default from running.
  #[must_use]
  pub const fn verdict(self) -> Prevent {
    match self {
      Self::Continue => Prevent::None,
      Self::Stop => Prevent::Bubbling,
    }
  }
}

#[derive(Default)]
struct Handlers {
  activate: Option<Rc<dyn Fn()>>,
  pointer: Option<Handler<PointerEvent>>,
  key: Option<Handler<KeyEvent>>,
  follow: Option<Handler<str>>,
  focus: Option<Rc<dyn Fn(bool)>>,
  size: Option<Rc<dyn Fn(Size)>>,
  dismiss: Option<Rc<dyn Fn()>>,
  context_menu: Option<Handler<ContextMenuEvent>>,
  drop: Option<DropHandler>,
}

struct Entry<R: Realization> {
  spec: NodeSpec,
  native: Option<R::Node>,
  scope: ScopeChain,
  states: Signal<StateSet>,
  control: Option<NodeId>,
  control_state: Option<Signal<ControlState>>,
  part: Option<Part>,
  dispatch: Option<Rc<ControlDispatch>>,
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

fn insert_entry<R: Realization>(
  spec: NodeSpec,
  scope: &ScopeChain,
  control: Option<NodeId>,
) -> NodeId {
  let control_state = spec
    .is_control()
    .then(|| Signal::new(ControlState::DEFAULT));

  with::<R, _>(|tree| {
    let id = tree.scene.insert(Entry {
      spec,
      native: None,
      scope: scope.clone(),
      states: Signal::new(StateSet::EMPTY),
      control,
      control_state,
      part: None,
      dispatch: None,
      record: None,
      controls: HashMap::new(),
      sink: None,
      handlers: Handlers::default(),
      interest: Interest::default(),
      access: Vec::new(),
      access_version: None,
    });

    if spec.is_control()
      && let Some(entry) = tree.scene.get_mut(id)
    {
      entry.control = Some(id);
    }

    id
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

/// Creates a node of `spec` in `scope`, built inside `control` (the nearest enclosing control,
/// whose states its control-scoped conditions resolve against; a control is its own),
/// realized at once where its kind is ([`realizes`]), forgotten when the current owner is
/// disposed.
pub fn create<R: Realization>(
  spec: NodeSpec,
  scope: &ScopeChain,
  control: Option<NodeId>,
) -> NodeId {
  let id = insert_entry::<R>(spec, scope, control);

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
  let id = insert_entry::<R>(spec, scope, None);
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
        entry.dispatch = None;
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

/// What a node's state conditions resolve against: its own reported states, and, for a node
/// built inside a control, the control's reported states and its [`ControlState`].
#[derive(Clone, Copy)]
struct Resolution {
  own: Signal<StateSet>,
  control: Option<(Signal<StateSet>, Option<Signal<ControlState>>)>,
}

impl Resolution {
  /// Whether `state` holds now, subscribing the running effect.
  fn holds(&self, state: State) -> bool {
    match &self.control {
      Some((reported, control)) if state.follows_control() => {
        reported.get().contains(state) || control.is_some_and(|control| control.get().holds(state))
      }
      _ if state.is_control_state() => false,
      _ => self.own.get().contains(state),
    }
  }

  /// Every state that holds now, subscribing the running effect.
  fn holding(&self) -> StateSet {
    State::ALL
      .into_iter()
      .filter(|state| self.holds(*state))
      .collect()
  }
}

fn resolution<R: Realization>(id: NodeId) -> Option<Resolution> {
  with::<R, _>(|tree| {
    let entry = tree.scene.get(id)?;
    let control = entry.control.and_then(|control| {
      let control = tree.scene.get(control)?;

      Some((control.states, control.control_state))
    });

    Some(Resolution {
      own: entry.states,
      control,
    })
  })
}

/// Asks the realization for `states` at `id`, beside what it reports there already.
fn watch_states<R: Realization>(id: NodeId, states: StateSet) {
  if states.is_empty() {
    return;
  }

  let watched = with::<R, _>(|tree| {
    let entry = tree.scene.get_mut(id)?;
    let grown = entry.interest.states.union(states);

    if grown == entry.interest.states {
      return None;
    }

    entry.interest.states = grown;

    Some((entry.native.clone()?, entry.interest))
  });

  if let Some((native, interest)) = watched {
    R::watch(&native, interest);
  }
}

/// Gives a node its record: one effect keeps its compiled style on the realization and, on
/// [`LayoutOwner::Engine`] platforms, one keeps its resolved layout on its host container.
///
/// Its state conditions resolve against its own states, and, where it is built inside a
/// control, its focus and press and the control's own states against the control's
/// ([`State::follows_control`]): the realization is asked for each where it is reported, at
/// the node or at its control, and the control-scoped states never.
pub fn set_record<R: Realization>(id: NodeId, record: Record) {
  let record = Rc::new(record);
  let Some((native, scope, control)) = with::<R, _>(|tree| {
    let entry = tree.scene.get_mut(id)?;

    entry.record = Some(Rc::clone(&record));

    Some((entry.native.clone()?, entry.scope.clone(), entry.control))
  }) else {
    return;
  };
  let Some(resolution) = resolution::<R>(id) else {
    return;
  };
  let engine = R::LAYOUT == LayoutOwner::Engine;
  let styled = move |group: Group| !engine || matches!(group, Group::Box | Group::Run);
  let laid_out = move |group: Group| matches!(group, Group::Layout | Group::Placement);
  let mut watched = record.states(laid_out);

  if !R::KEEP.states() {
    watched = watched.union(record.states(styled));
  }

  let (mut own, mut at_control) = (StateSet::EMPTY, StateSet::EMPTY);

  for state in watched.iter() {
    match control {
      Some(_) if state.is_control_state() => {}
      Some(_) if state.follows_control() => at_control = at_control.with(state),
      _ => own = own.with(state),
    }
  }

  watch_states::<R>(id, own);

  if let Some(control) = control {
    watch_states::<R>(control, at_control);
  }

  {
    let (record, native, scope) = (Rc::clone(&record), native.clone(), scope.clone());
    let last = RefCell::new(None::<CompiledStyle>);

    Effect::new(move || {
      let compiled = record.compile(R::KEEP, styled, &scope, &|state| resolution.holds(state));
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
      let compiled = record.compile(Keep::Nothing, laid_out, &scope, &|state| {
        resolution.holds(state)
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

/// Keeps `content` at what `derive` returns, erased: [`bind`] for a caller holding the
/// content's name rather than its typed key (a behavior through its handle).
///
/// # Panics
///
/// Where the node's kind does not take `content`, or `derive` returns another type.
pub fn bind_value<R: Realization>(id: NodeId, content: Content, derive: Rc<dyn Fn() -> PropValue>) {
  let last = RefCell::new(None::<PropValue>);

  check_takes::<R>(id, content);
  Effect::new(move || {
    let value = derive();
    let mut last = last.borrow_mut();

    if last.as_ref() != Some(&value) {
      untrack(|| writing(|| write_content::<R>(id, content, &value)));
      *last = Some(value);
    }
  });
}

/// Makes `content` controlled, erased: [`control`] for a caller holding the content's name
/// rather than its typed key (a behavior through its handle).
///
/// # Panics
///
/// Where the node's kind does not take `content`.
pub fn control_value<R: Realization>(
  id: NodeId,
  content: Content,
  derive: Rc<dyn Fn() -> PropValue>,
  handler: Rc<dyn Fn(PropValue)>,
) {
  check_takes::<R>(id, content);

  let controller = Controller::<R>::new(
    id,
    content,
    Box::new(move || derive()),
    Some(Box::new(move |value| handler(value))),
  );

  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.controls.insert(content, Rc::clone(&controller));
    }
  });

  Effect::new(move || {
    let truth = controller.tracked_truth();

    untrack(|| controller.show(&truth));
  });
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

/// Runs `activate` when the box is clicked, or activated from the keyboard or by a reader.
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

/// Runs `menu` when a context menu is asked for at the node or beneath it.
pub fn handle_context_menu<R: Realization, B: Into<Bubbling>>(
  id: NodeId,
  menu: impl Fn(&ContextMenuEvent) -> B + 'static,
) {
  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.handlers.context_menu = Some(Rc::new(move |event| menu(event).into()));
    }
  });
}

/// Runs `drop` with what is dropped on the node or beneath it, text or URIs: it answers
/// whether it took it. The realization accepts what is dragged over the node.
pub fn handle_drop<R: Realization>(id: NodeId, drop: impl Fn(&Dropped) -> bool + 'static) {
  watch_now::<R>(id, |entry| {
    entry.handlers.drop = Some(Rc::new(drop));
    entry.interest.drop = true;
  });
}

/// Runs `key` with the key events while focus is on the node or beneath it, after the
/// behaviors of the controls between, unless one stopped it.
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

/// Runs `dismiss` when the overlay `id` is dismissed: a press outside it and its anchor, or
/// the platform's own dismissal.
pub fn handle_dismiss<R: Realization>(id: NodeId, dismiss: impl Fn() + 'static) {
  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.handlers.dismiss = Some(Rc::new(dismiss));
    }
  });
}

/// Registers what the control `id` answers: its behavior's handlers. The realization is asked
/// for its keys, its pointer events and its focus.
pub fn handle_control<R: Realization>(id: NodeId, dispatch: ControlDispatch) {
  watch_now::<R>(id, |entry| {
    entry.dispatch = Some(Rc::new(dispatch));
    entry.interest.pointer = true;
    entry.interest.keys = true;
    entry.interest.focus = true;
  });
}

/// Keeps the control `id`'s own state at what `derive` returns: what the control-scoped
/// conditions of every node built inside it resolve against.
pub fn set_control_state<R: Realization>(id: NodeId, derive: impl Fn() -> ControlState + 'static) {
  let Some(state) = with::<R, _>(|tree| tree.scene.get(id)?.control_state) else {
    return;
  };

  Effect::new(move || {
    let next = derive();

    if state.get_untracked() != next {
      state.set(next);
    }
  });
}

/// Records `id` as `part` of the control it is built inside.
pub fn set_part<R: Realization>(id: NodeId, part: Part) {
  with::<R, _>(|tree| {
    if let Some(entry) = tree.scene.get_mut(id) {
      entry.part = Some(part);
    }
  });
}

/// The part `id` was recorded as.
#[must_use]
pub fn part_of<R: Realization>(id: NodeId) -> Option<Part> {
  with::<R, _>(|tree| tree.scene.get(id)?.part)
}

/// The control `id` is built inside (a control is its own), where it is built inside one.
#[must_use]
pub fn control_of<R: Realization>(id: NodeId) -> Option<NodeId> {
  with::<R, _>(|tree| tree.scene.get(id)?.control)
}

/// The control's first node recorded as `part`, in the scene's order; `Part::Root` the
/// control's own node.
#[must_use]
pub fn find_part<R: Realization>(control: NodeId, part: Part) -> Option<NodeId> {
  if part == Part::Root {
    return Some(control);
  }

  with::<R, _>(|tree| {
    tree
      .scene
      .descendants(control)
      .into_iter()
      .map(|(id, _)| id)
      .find(|id| {
        tree
          .scene
          .get(*id)
          .is_some_and(|entry| entry.part == Some(part) && entry.control == Some(control))
      })
  })
}

/// `id`'s rectangle in window pixels, as last laid out.
#[must_use]
pub fn frame<R: Realization>(id: NodeId) -> Option<Rect> {
  native::<R>(id).and_then(|native| R::frame(&native))
}

/// Whether `op` is offered on `id` now ([`Realization::clipboard_availability`]): absent where
/// it is not realized.
#[must_use]
pub fn clipboard_availability<R: Realization>(id: NodeId, op: ClipboardOp) -> Availability {
  native::<R>(id).map_or(Availability::Absent, |native| {
    R::clipboard_availability(&native, op)
  })
}

/// Carries `op` out on `id` ([`Realization::clipboard`]), where it is realized.
pub fn clipboard<R: Realization>(id: NodeId, op: ClipboardOp) {
  if let Some(native) = native::<R>(id) {
    R::clipboard(&native, op);
  }
}

/// Moves focus to `id`, where it is realized.
pub fn focus<R: Realization>(id: NodeId) {
  if let Some(native) = native::<R>(id) {
    R::focus(&native);
  }
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
/// applying is told what to apply instead and one that applied first what to repair to. A key
/// climbs from the focused node, each control's behavior answering before the box's handler,
/// until a verdict stops it; a pointer event climbs from the deepest node and stops at a
/// control or an acting box (a view that handles an interaction owns it), a disabled control
/// swallowing it; a click and a reader's request are the owner's alone. The answer folds every
/// verdict met, and the realization runs its default unless it prevents it.
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
    Report::Click => click::<R>(id),
    Report::Access(request) => access::<R>(id, request),
    Report::Dismiss => {
      let dismiss = with::<R, _>(|tree| tree.scene.get(id)?.handlers.dismiss.clone());

      if let Some(dismiss) = dismiss {
        untrack(|| dismiss());
      }

      Answer::None
    }
    Report::Follow(destination) => {
      let path = with::<R, _>(|tree| {
        std::iter::once(id)
          .chain(tree.scene.ancestors(id))
          .filter_map(|node| tree.scene.get(node)?.handlers.follow.clone())
          .collect::<Vec<_>>()
      });
      let mut prevented = Prevent::None;

      for follow in path {
        prevented = prevented.and(Prevent::Platform);

        if untrack(|| follow(&destination)) == Bubbling::Stop {
          break;
        }
      }

      Answer::Event { prevented }
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
      let (focus, dispatch) = with::<R, _>(|tree| {
        let entry = tree.scene.get(id)?;

        Some((entry.handlers.focus.clone(), entry.dispatch.clone()))
      })
      .unwrap_or_default();

      if let Some(focus) = focus {
        untrack(|| focus(focused));
      }

      if let (false, Some(dispatch)) = (focused, dispatch) {
        untrack(|| (dispatch.blur)());
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
    Report::Pointer(event) => pointer::<R>(id, &event),
    Report::Size(size) => {
      let handler = with::<R, _>(|tree| tree.scene.get(id)?.handlers.size.clone());

      if let Some(handler) = handler {
        untrack(|| handler(size));
      }

      Answer::None
    }
    Report::Key(event) => key::<R>(id, &event),
    Report::ContextMenu(event) => context_menu::<R>(id, &event),
    Report::Drop(event) => drop::<R>(id, &event),
  }
}

/// A context menu asked for at `id`: each handler from it up runs until one stops it; where
/// one ran, the platform's own menu is prevented.
fn context_menu<R: Realization>(id: NodeId, event: &ContextMenuEvent) -> Answer {
  let path = with::<R, _>(|tree| {
    std::iter::once(id)
      .chain(tree.scene.ancestors(id))
      .filter_map(|node| tree.scene.get(node)?.handlers.context_menu.clone())
      .collect::<Vec<_>>()
  });
  let mut prevented = Prevent::None;

  for menu in path {
    prevented = prevented.and(Prevent::Platform);

    if untrack(|| menu(event)) == Bubbling::Stop {
      break;
    }
  }

  Answer::Event { prevented }
}

/// A drop at `id`: each drop handler from it up is offered it until one takes it, which the
/// answer's `prevented` says.
fn drop<R: Realization>(id: NodeId, event: &DropEvent) -> Answer {
  let path = with::<R, _>(|tree| {
    std::iter::once(id)
      .chain(tree.scene.ancestors(id))
      .filter_map(|node| tree.scene.get(node)?.handlers.drop.clone())
      .collect::<Vec<_>>()
  });

  for drop in path {
    if untrack(|| drop(&event.dropped)) {
      return Answer::Event {
        prevented: Prevent::Platform,
      };
    }
  }

  Answer::Event {
    prevented: Prevent::None,
  }
}

/// The dispatch of `id`'s control and its box's activation, where it has them.
type ClickHandlers = (Option<Rc<ControlDispatch>>, Option<Rc<dyn Fn()>>);

fn owner_of<R: Realization>(id: NodeId) -> ClickHandlers {
  with::<R, _>(|tree| {
    let entry = tree.scene.get(id)?;

    Some((entry.dispatch.clone(), entry.handlers.activate.clone()))
  })
  .unwrap_or_default()
}

fn click<R: Realization>(id: NodeId) -> Answer {
  let prevented = match owner_of::<R>(id) {
    (Some(dispatch), _) if untrack(|| (dispatch.disabled)()) => Prevent::All,
    (Some(dispatch), _) => untrack(|| (dispatch.click)()),
    (None, Some(activate)) => {
      untrack(|| activate());

      Prevent::All
    }
    (None, None) => Prevent::None,
  };

  Answer::Event { prevented }
}

fn access<R: Realization>(id: NodeId, request: AccessRequest) -> Answer {
  let took = match owner_of::<R>(id) {
    (Some(dispatch), _) if untrack(|| (dispatch.disabled)()) => false,
    (Some(dispatch), _) => {
      untrack(|| (dispatch.access)(request))
        || (request == AccessRequest::Activate && {
          untrack(|| (dispatch.click)());

          true
        })
    }
    (None, Some(activate)) if request == AccessRequest::Activate => {
      untrack(|| activate());

      true
    }
    (None, _) => false,
  };

  Answer::Event {
    prevented: if took { Prevent::All } else { Prevent::None },
  }
}

fn key<R: Realization>(id: NodeId, event: &KeyEvent) -> Answer {
  let path = with::<R, _>(|tree| {
    std::iter::once(id)
      .chain(tree.scene.ancestors(id))
      .filter_map(|node| {
        let entry = tree.scene.get(node)?;

        Some((entry.dispatch.clone(), entry.handlers.key.clone()))
      })
      .collect::<Vec<_>>()
  });
  let mut prevented = Prevent::None;

  for (dispatch, handler) in path {
    let mut verdict = Prevent::None;

    if let Some(dispatch) = dispatch
      && !untrack(|| (dispatch.disabled)())
    {
      verdict = untrack(|| (dispatch.key)(event));
    }

    if let Some(handler) = handler {
      verdict = verdict.and(untrack(|| handler(event)).verdict());
    }

    prevented = prevented.and(verdict);

    if verdict.stops() {
      break;
    }
  }

  Answer::Event { prevented }
}

fn pointer<R: Realization>(id: NodeId, event: &PointerEvent) -> Answer {
  let path = with::<R, _>(|tree| {
    std::iter::once(id)
      .chain(tree.scene.ancestors(id))
      .filter_map(|node| {
        let entry = tree.scene.get(node)?;

        Some((
          entry.spec.is_interactive(),
          entry.dispatch.clone(),
          entry.handlers.pointer.clone(),
        ))
      })
      .collect::<Vec<_>>()
  });
  let mut prevented = Prevent::None;

  for (interactive, dispatch, handler) in path {
    let mut verdict = Prevent::None;

    if let Some(dispatch) = dispatch {
      if untrack(|| (dispatch.disabled)()) {
        prevented = Prevent::All;

        break;
      }

      verdict = untrack(|| (dispatch.pointer)(event));
    }

    if let Some(handler) = handler {
      verdict = verdict.and(untrack(|| handler(event)).verdict());
    }

    prevented = prevented.and(verdict);

    if verdict.stops() || interactive {
      break;
    }
  }

  Answer::Event { prevented }
}

/// The states `id`'s style resolves against now, its enclosing control's included,
/// subscribing the running effect to them.
#[must_use]
pub fn resolved_states<R: Realization>(id: NodeId) -> StateSet {
  resolution::<R>(id).map_or(StateSet::EMPTY, |resolution| resolution.holding())
}

#[cfg(test)]
mod tests {
  use super::Resolution;
  use crate::{
    Owner, Signal, State, StateSet,
    seam::{access::Checked, control::ControlState},
  };

  /// A box outside every control answers its own states and no control's; a part answers
  /// its own hover and its control's focus, press and states; a control answers its own.
  #[test]
  fn a_part_resolves_focus_press_and_control_states_against_its_control() {
    Owner::root().run(|| {
      let control_reported = Signal::new(StateSet::EMPTY.with(State::FocusVisible));
      let control_state = Signal::new(ControlState {
        checked: Checked::On,
        pressed: true,
        ..ControlState::DEFAULT
      });
      let part_reported = Signal::new(StateSet::EMPTY.with(State::Hovered));
      let part = Resolution {
        own: part_reported,
        control: Some((control_reported, Some(control_state))),
      };
      let control = Resolution {
        own: control_reported,
        control: Some((control_reported, Some(control_state))),
      };
      let outside = Resolution {
        own: Signal::new(
          StateSet::EMPTY
            .with(State::FocusVisible)
            .with(State::Hovered),
        ),
        control: None,
      };

      assert!(part.holds(State::Hovered));
      assert!(part.holds(State::FocusVisible));
      assert!(part.holds(State::Pressed));
      assert!(part.holds(State::Checked));
      assert!(!part.holds(State::Disabled));
      assert!(!control.holds(State::Hovered));
      assert!(control.holds(State::Checked));
      assert!(outside.holds(State::FocusVisible));
      assert!(outside.holds(State::Hovered));
      assert!(!outside.holds(State::Checked));

      control_state.set(ControlState::DEFAULT);

      assert!(!part.holds(State::Checked));
      assert!(!part.holds(State::Pressed));
    });
  }
}
