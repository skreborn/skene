//! # skene-gui-linux
//!
//! skene-gui's Linux realization, drawn by Skene: Linux composites windows and nothing inside
//! them, so Skene owns the whole window. It speaks the display server's protocols itself
//! (Wayland through smithay-client-toolkit, X11 through x11rb), lays the scene out with the
//! foundation's engine ([`LayoutOwner::Engine`]), resolves every style in the foundation
//! ([`Keep::Nothing`]), and paints with vello_cpu and Parley (FreeType's glyph masks under the
//! reader's font settings) into shared memory. With the `gpu` feature, a window an adapter
//! presents to paints the same scene with vello's GPU renderer over wgpu's Vulkan and
//! presents through the swapchain instead, the CPU path its floor ([`testing::PathChoice`]).
//!
//! Its modules are private: the window it draws is reached through the display server and a
//! reader alone, never by a caller.
//!
//! - `node`: the realized tree, what the realization keeps per node.
//! - `look`, `palette`: a node's resolved look and run properties, the platform's colors the
//!   built-in controls are drawn in, and the defaults drawn where a view sets none.
//! - `motion`: transitions and keyframe animations on the window's frame clock.
//! - `text`, `layout`: paragraphs through the foundation's engine contract, and the layout
//!   engine's host over the realized tree.
//! - `entry`, `overlay`: the realization's own text view, and an overlay's surface.
//! - `adornment`: what the window shows on surfaces of its own that take no input: a drag's
//!   icon, a touch's selection handles and its magnifier.
//! - `paint`, `chrome`: the painter, the client-side decorations where the compositor draws
//!   none; with the `gpu` feature, `gpu`: the GPU path, vello's GPU renderer over wgpu,
//!   chosen once per window where an adapter presents to it.
//! - `access`: the reader's tree through AccessKit.
//! - `shell`: the window both window systems share: frames, hit testing, hover, press and focus,
//!   Tab order, keys and wheels dispatched through the foundation.
//! - `keys`, `ime`, `kinetic`, `runtime`, `environment`: the keyboard through xkbcommon, the
//!   input methods, a fling's motion, the loop's timers and tasks, the reader's preferences
//!   from the settings portal.
//! - `wayland`, `x11`, `xdnd`: the window systems and X11's drag and drop; `window`: a window on
//!   either, its loop turned a step at a time.
//!
//! What it offers beside the [`Linux`] realization:
//!
//! - [`clipboard`]: the clipboard and the primary selection, which `skene_gui`'s Linux
//!   platform module carries.
//! - [`trace`]: what a harness reads (frame costs, states, screenshots, the pointer).
//! - [`testing`]: `skene_gui::testing` on Linux: the driver reading through AT-SPI and acting
//!   through real input, and the test entry.

mod access;
mod adornment;
mod chrome;
pub mod clipboard;
mod entry;
mod environment;
#[cfg(feature = "gpu")]
mod gpu;
mod ime;
mod keys;
mod kinetic;
mod layout;
mod look;
mod motion;
mod node;
mod overlay;
mod paint;
mod palette;
mod runtime;
mod shell;
pub mod testing;
mod text;
pub mod trace;
mod wayland;
mod window;
mod x11;
mod xdnd;

use std::{process::ExitCode, rc::Rc, time::Duration};

use skene_foundation::{
  NodeId, Owner, Scheme, State, SystemColors,
  layout::{Rect, ResolvedLayout},
  seam::{
    LayoutOwner, Realization,
    access::AccessFacts,
    content::{Content, PropValue, Unit},
    control::{Availability, ClipboardOp},
    node::NodeSpec,
    report::{Interest, Report},
    runtime::{Cancel, Task},
    style::{CompiledStyle, Keep, KeptDefinition, KeptLeaf, ScopeId},
    tree,
  },
};

use crate::node::{Drawn, Node};

/// The Linux realization: Skene draws the window.
pub struct Linux;

impl Realization for Linux {
  type Node = Node;
  type Widget = Node;

  const LAYOUT: LayoutOwner = LayoutOwner::Engine;
  const KEEP: Keep = Keep::Nothing;
  const UNIT: Unit = Unit::Utf8;
  const SCOPE_NODES: bool = false;
  const DRAWS_SCROLL_BARS: bool = true;

  fn create(id: NodeId, spec: &NodeSpec) -> Node {
    let node = Drawn::new(id, *spec);

    if node.is_overlay() {
      node.set_hidden(true);
      overlay::register(&node);
    }

    node
  }

  fn adopt(id: NodeId, widget: Node, _: &NodeSpec) -> Node {
    widget.id.set(id);

    widget
  }

  fn widget(node: &Node) -> Node {
    Rc::clone(node)
  }

  fn insert(parent: &Node, child: &Node, index: usize) {
    if let Some(old) = child.parent() {
      crate::layout::mark(&old, true);
    }

    parent.insert(child, index);
    crate::layout::mark(parent, true);
    shell::invalidate_prepare();
    shell::invalidate_layout();
  }

  fn relocate(parent: &Node, child: &Node, index: usize) {
    if let Some(old) = child.parent() {
      crate::layout::mark(&old, true);
    }

    parent.insert(child, index);
    crate::layout::mark(parent, true);
    shell::invalidate_layout();
  }

  fn set_hidden(node: &Node, hidden: bool) {
    if node.is_hidden() != hidden {
      if hidden && !node.is_overlay() {
        shell::will_leave(node, true);
      }

      node.set_hidden(hidden);

      if node.is_overlay() {
        shell::invalidate_overlays();
      } else {
        crate::layout::mark(node, true);
        shell::invalidate_layout();
      }
    }
  }

  fn remove(node: &Node) {
    shell::will_leave(node, false);

    if let Some(parent) = node.parent() {
      crate::layout::mark(&parent, true);
    }

    node.remove();
    shell::invalidate_layout();
  }

  fn apply_style(node: &Node, style: &CompiledStyle) {
    let (look, run) = node.style_now(style);

    node.show_style(look, run);
  }

  fn apply_slots(_: &Node, _: &[KeptLeaf]) {}

  fn apply_layout(node: &Node, layout: &ResolvedLayout) {
    *node.layout.borrow_mut() = node.layout_now(layout);
    crate::layout::mark(node, true);
    shell::invalidate_layout();
  }

  fn define_tokens(_: ScopeId, _: Option<&Node>, _: &[KeptDefinition]) {}

  fn write(node: &Node, content: Content, value: &PropValue) {
    shell::write(node, content, value);
  }

  fn read(node: &Node, content: Content) -> Option<PropValue> {
    if content == Content::ScrollOffset {
      let (x, y) = node.scroll.get();

      return Some(PropValue::Offset(
        skene_foundation::seam::content::ScrollOffset::new(x, y),
      ));
    }

    if content == Content::Edit
      && let Ok(entry) = node.entry.try_borrow()
      && let Some(entry) = entry.as_ref()
    {
      return Some(PropValue::Edit(skene_foundation::seam::content::Edit {
        text: entry.editor.text().into(),
        selection: Some(entry.editor.selection()),
      }));
    }

    node.contents.borrow().read(content)
  }

  fn composing(node: &Node) -> bool {
    node.composing.get()
  }

  fn apply_access(
    node: &Node,
    _: &AccessFacts,
    next: &AccessFacts,
    _: &dyn Fn(NodeId) -> Option<Node>,
  ) {
    *node.access.borrow_mut() = next.clone();
    access::invalidate();
  }

  fn watch(node: &Node, interest: Interest) {
    let previous = node.interest.replace(interest);
    let fresh = interest
      .states
      .iter()
      .filter(|state| !previous.states.contains(*state));

    for state in fresh.collect::<Vec<State>>() {
      if node.states.get().contains(state) {
        tree::report::<Self>(node.id.get(), Report::State { state, holds: true });
      }
    }

    if interest.size && !previous.size {
      node.reported.set(None);
      shell::invalidate_layout();
    }
  }

  fn focus(node: &Node) {
    shell::request_focus(Some(Rc::clone(node)));
    runtime::inbox().send(runtime::Message::Wake);
  }

  fn frame(node: &Node) -> Option<Rect> {
    (node.is_alive() && node.is_shown()).then(|| node.window_rect())
  }

  fn system_colors() -> SystemColors {
    palette::system_colors()
  }

  fn painting_available() -> bool {
    true
  }

  fn set_scheme(choice: Option<Scheme>) {
    environment::set_scheme(choice);
  }

  fn spawn_local(task: Task) -> Cancel {
    runtime::spawn(task)
  }

  fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Cancel {
    runtime::schedule(delay, action)
  }

  fn clipboard_availability(node: &Node, op: ClipboardOp) -> Availability {
    node.clipboard_availability(op)
  }

  fn clipboard(node: &Node, op: ClipboardOp) {
    node.clipboard_op(op);
  }

  /// GTK's entry menu's editing entries, which it follows with an emoji chooser.
  fn text_menu() -> &'static [ClipboardOp] {
    &[
      ClipboardOp::Cut,
      ClipboardOp::Copy,
      ClipboardOp::Paste,
      ClipboardOp::Delete,
      ClipboardOp::SelectAll,
    ]
  }

  /// As GTK labels them.
  fn operation_label(op: ClipboardOp) -> &'static str {
    match op {
      ClipboardOp::Cut => "Cut",
      ClipboardOp::Copy => "Copy",
      ClipboardOp::Paste => "Paste",
      ClipboardOp::Delete => "Delete",
      ClipboardOp::SelectAll => "Select All",
    }
  }

  fn start(app_id: &str, root: Box<dyn FnOnce() -> NodeId>) -> ExitCode {
    start(app_id, root)
  }
}

/// The realized node of scene node `id`, where it is realized.
pub(crate) fn node_of(id: NodeId) -> Option<Node> {
  tree::native::<Linux>(id)
}

/// Which display server the window goes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowSystem {
  /// Wayland.
  Wayland,
  /// X11.
  X11,
}

impl WindowSystem {
  /// The window system `SKENE_WINDOW_SYSTEM` names (`wayland` or `x11`), else Wayland where
  /// `WAYLAND_DISPLAY` is set, else X11 where `DISPLAY` is.
  pub fn choose() -> Option<Self> {
    match std::env::var("SKENE_WINDOW_SYSTEM").as_deref() {
      Ok("wayland") => return Some(Self::Wayland),
      Ok("x11") => return Some(Self::X11),
      _ => {}
    }

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
      Some(Self::Wayland)
    } else if std::env::var_os("DISPLAY").is_some() {
      Some(Self::X11)
    } else {
      None
    }
  }
}

/// The window's size before the window manager says otherwise.
pub(crate) const DEFAULT_SIZE: (u32, u32) = (480, 640);

/// The window's title and size an app opens with: `SKENE_TITLE` and `SKENE_SIZE` (`WxH`)
/// where set, else the app id's last part and [`DEFAULT_SIZE`].
fn window_settings(app_id: &str) -> (String, (u32, u32)) {
  let title = std::env::var("SKENE_TITLE")
    .unwrap_or_else(|_| app_id.rsplit('.').next().unwrap_or(app_id).to_owned());
  let size = std::env::var("SKENE_SIZE")
    .ok()
    .and_then(|size| {
      let (width, height) = size.split_once('x')?;

      Some((width.parse().ok()?, height.parse().ok()?))
    })
    .unwrap_or(DEFAULT_SIZE);

  (title, size)
}

/// Opens the app's window on the display server in use, builds `root` inside it under an
/// owner that lives as long as the window, and runs the loop until the app quits.
pub(crate) fn start(app_id: &str, root: Box<dyn FnOnce() -> NodeId>) -> ExitCode {
  let Some(system) = WindowSystem::choose() else {
    eprintln!("skene: no display: neither WAYLAND_DISPLAY nor DISPLAY is set");

    return ExitCode::FAILURE;
  };

  text::preload();

  let (title, size) = window_settings(app_id);

  if !environment::follow_portal() {
    trace::log(|| "[portal] no settings portal answered".to_owned());
  }

  environment::apply();
  // The threads that shape text beside this one ready their fonts while the app is built.
  text::engine().start_shapers();

  let mut shell = shell::Shell::new(&title, size.0, size.1);
  let owner = Owner::root();
  let root = owner.run(root);

  if let Some(node) = tree::native::<Linux>(root) {
    shell.set_root(node);
  }

  trace::log(|| format!("[start] {system:?} {app_id} {}x{}", size.0, size.1));

  let result = window::Window::open(system, shell, app_id).and_then(|mut window| {
    while !window.shell().quit {
      window.step(None)?;
    }

    Ok(())
  });

  owner.dispose();

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("skene: {error}");

      ExitCode::FAILURE
    }
  }
}

/// What the loop does between waits, whichever window system runs it: fires timers and tasks,
/// applies what other threads posted, settles focus. Returns what was posted for the window system
/// (its input method's events), the accessibility layer's requests carried out here.
pub(crate) fn turn(shell: &mut shell::Shell) -> Vec<runtime::Message> {
  let mut rest = Vec::new();

  for message in runtime::inbox().drain() {
    match message {
      runtime::Message::Setting(namespace, key, value) => {
        environment::setting_changed(&namespace, &key, &value);
      }
      runtime::Message::Access(request) => access::handle(shell, request),
      runtime::Message::Wake => {}
      #[cfg(feature = "gpu")]
      runtime::Message::DeviceLost => {
        shell::invalidate_all();

        for level in 0..shell.popup_count() {
          if let Some(open) = shell.popup_mut(level) {
            open.invalidate();
          }
        }
      }
      message @ runtime::Message::Ime(_) => rest.push(message),
    }
  }

  runtime::run_due();
  shell.touch_timers();
  shell.settle_focus();
  shell.sync_overlays();
  access::sync(shell);

  rest
}
