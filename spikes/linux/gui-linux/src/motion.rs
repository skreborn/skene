//! Motion on the window's frame clock: each node whose style names transitions or keyframe
//! animations keeps the foundation's runner ([`Motion`]), fed where the shared half reports
//! the node's resolved style changed ([`skene_foundation::seam::Realization::apply_style`]),
//! whatever changed it (a condition flipping, a token redefined, a derived value written), and
//! presented once a frame at the frame's clock reading until nothing runs. The reactive graph
//! sees none of it, and a window with nothing in motion asks for no frame.

use std::{
  cell::RefCell,
  rc::{Rc, Weak},
  sync::OnceLock,
  time::Instant,
};

use skene_foundation::{
  Environment,
  motion::Motion,
  seam::{
    record::{CompiledAnimation, Prop, Resolved, Transition},
    style::CompiledStyle,
  },
  untrack,
};

use crate::{
  look::{self, Look, RunProps},
  node::{Drawn, Node},
  shell,
};

/// A node's motion: what its style resolves to now, the transitions and animations it
/// names, and the runner moving between them.
pub struct Moving {
  targets: Vec<(Prop, Resolved)>,
  transitions: Vec<Transition>,
  animations: Vec<CompiledAnimation>,
  motion: Motion,
}

thread_local! {
  static MOVING: RefCell<Vec<Weak<Drawn>>> = const { RefCell::new(Vec::new()) };
}

/// The frame clock's reading: seconds since the process first read it.
pub fn now() -> f64 {
  static EPOCH: OnceLock<Instant> = OnceLock::new();

  EPOCH.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// Whether the reader prefers reduced motion, read without subscribing.
fn reduced() -> bool {
  untrack(|| Environment::current().reduced_motion.get())
}

/// Whether some node's motion runs, so the next frame is due on the frame clock.
pub fn running() -> bool {
  MOVING.with(|moving| !moving.borrow().is_empty())
}

fn keep_moving(node: &Node) {
  MOVING.with(|moving| {
    let mut moving = moving.borrow_mut();

    if !moving
      .iter()
      .any(|held| std::ptr::eq(held.as_ptr(), Rc::as_ptr(node)))
    {
      moving.push(Rc::downgrade(node));
    }
  });
}

impl Drawn {
  /// The look and run properties `style` gives this node now: its resolved values, with its
  /// transitions started from what it showed and its keyframe animations laid over, at the
  /// frame clock's reading. A node in motion asks for frames until its motion rests.
  pub fn style_now(self: &Rc<Self>, style: &CompiledStyle) -> (Look, RunProps) {
    let targets = look::targets(style);
    let mut slot = self.motion.borrow_mut();

    if slot.is_none() && style.transitions.is_empty() && style.animations.is_empty() {
      return look::split(&targets);
    }

    let (now, reduced) = (now(), reduced());
    let moving = slot.get_or_insert_with(|| {
      Box::new(Moving {
        targets: targets.clone(),
        transitions: Vec::new(),
        animations: Vec::new(),
        motion: Motion::new(),
      })
    });

    moving
      .motion
      .update(&moving.targets, &targets, &style.transitions, now, reduced);

    let fresh = moving.animations != style.animations;

    if fresh {
      moving.motion.restart();
      moving.animations.clone_from(&style.animations);
    }

    moving.targets = targets;
    moving.transitions.clone_from(&style.transitions);

    let (shown, running) = moving
      .motion
      .present(&moving.targets, &moving.animations, now, reduced);

    if fresh {
      // Animations count from the first frame that paints them.
      moving.motion.restart();
    }

    if running {
      keep_moving(self);
    } else if moving.transitions.is_empty() && moving.animations.is_empty() {
      *slot = None;
    }

    look::split(&shown)
  }

  /// Shows `look` and `run`, asking for what each change needs: the area painted before and
  /// after, a layout where the border's width moved, the paragraphs beneath where a run
  /// property did.
  pub fn show_style(self: &Rc<Self>, look: Look, run: RunProps) {
    if *self.look.borrow() == look && *self.run.borrow() == run {
      return;
    }

    if self.is_shown() {
      // What it painted before, which its new look may no longer cover.
      shell::invalidate_rect(shell::reach_rect(self));
    }

    let relayout = self.look.borrow().border_width() != look.border_width();
    let restyled = *self.run.borrow() != run;

    *self.look.borrow_mut() = look;

    if restyled {
      *self.run.borrow_mut() = run;
      shell::invalidate_prepare_of(self);
    }

    if relayout {
      shell::invalidate_layout();
    }

    shell::invalidate_paint(self);
  }
}

/// Presents every node in motion at the clock reading `now`: what a frame does before it lays
/// out and paints. A node whose motion rests leaves the frame clock.
pub fn advance(now: f64) {
  let reduced = reduced();
  let nodes: Vec<Node> = MOVING.with(|moving| {
    std::mem::take(&mut *moving.borrow_mut())
      .into_iter()
      .filter_map(|node| node.upgrade())
      .filter(|node| node.is_alive())
      .collect()
  });

  for node in nodes {
    let presented = node.motion.borrow_mut().as_mut().map(|moving| {
      moving
        .motion
        .present(&moving.targets, &moving.animations, now, reduced)
    });
    let Some((shown, running)) = presented else {
      continue;
    };
    let (look, run) = look::split(&shown);

    node.show_style(look, run);

    if running {
      keep_moving(&node);
    }
  }
}
