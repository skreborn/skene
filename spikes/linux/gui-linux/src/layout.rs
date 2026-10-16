//! Layout: the foundation's engine over the realized tree. [`Host`] answers the engine's host
//! protocol for every realized node (a container through its own engine, a text through its
//! paragraph, a control through its metrics), a scroller lays its one child out at the height
//! it asks for, and [`prepare`] gives every paragraph the run properties it inherits before a
//! layout reads it.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{
  layout::{Axis, ContainerLayout, LayoutHost, Measurement, Rect, ResolvedLayout},
  seam::node::{LayoutKind, NodeKind},
  text::{AttributedText, MeasureText, Space},
  value::Inset,
};

use crate::{
  controls,
  look::RunProps,
  node::{Drawn, Node},
  palette::{DEFAULT_SIZE, Palette},
  text::{Change, TextCell, attributed_runs},
};

thread_local! {
  /// The layout epoch: measurements cached in an earlier one are stale.
  static EPOCH: Cell<u64> = const { Cell::new(1) };
  /// How many measurements were computed rather than read from a cache, since the thread
  /// started.
  static MEASURED: Cell<u64> = const { Cell::new(0) };
}

/// Starts a layout epoch: every cached measurement is stale.
pub fn next_epoch() {
  EPOCH.with(|epoch| epoch.set(epoch.get() + 1));
}

/// How many measurements were computed since the thread started.
pub fn measured() -> u64 {
  MEASURED.with(Cell::get)
}

/// The realized tree as the engine's host.
pub struct Host;

/// A node's own layout as its engine takes it: its border taking room inside its edges, as
/// its padding does.
fn own_layout(node: &Drawn) -> ResolvedLayout {
  let mut own = node.layout.borrow().clone();
  let border = node.look.borrow().border_width();

  if border > 0. {
    let padding = own.padding.unwrap_or(Inset {
      top: 0.,
      trailing: 0.,
      bottom: 0.,
      leading: 0.,
    });

    own.padding = Some(Inset {
      top: padding.top + border,
      trailing: padding.trailing + border,
      bottom: padding.bottom + border,
      leading: padding.leading + border,
    });
  }

  own
}

/// Runs `body` over `node`'s engine, its children and its own layout current.
///
/// The children and their placements are given once a layout epoch: nothing changes them
/// while a layout runs.
fn with_engine<T>(node: &Node, body: impl FnOnce(&mut ContainerLayout<Host>) -> T) -> T {
  let kind = node.layout_kind().unwrap_or(LayoutKind::Box);
  let epoch = EPOCH.with(Cell::get);
  let mut engine = node.engine.borrow_mut();
  let engine = engine.get_or_insert_with(|| ContainerLayout::new(kind, ResolvedLayout::default()));

  if node.engine_epoch.replace(epoch) != epoch {
    let children: Vec<(Node, ResolvedLayout)> = node
      .children()
      .into_iter()
      .map(|child| {
        let placement = child.layout.borrow().clone();

        (child, placement)
      })
      .collect();

    engine.set_own(own_layout(node));
    engine.set_children(children);
  }

  body(engine)
}

/// The one child a scroller scrolls.
fn scrolled(node: &Node) -> Option<Node> {
  node.children().into_iter().find(|child| !child.is_hidden())
}

fn measure_uncached(node: &Node, axis: Axis, for_size: Option<f32>) -> Measurement {
  match node.spec.kind {
    NodeKind::Layout(LayoutKind::Scroll {
      horizontal,
      vertical,
    }) => {
      let Some(child) = scrolled(node) else {
        return Measurement::default();
      };
      let measured = Host::measure(&child, axis, for_size);
      let scrolls = match axis {
        Axis::Horizontal => horizontal,
        Axis::Vertical => vertical,
      };

      if scrolls {
        Measurement {
          minimum: 0.,
          natural: measured.natural,
          baseline: None,
        }
      } else {
        measured
      }
    }
    NodeKind::Layout(_) => with_engine(node, |engine| engine.measure(axis, for_size)),
    NodeKind::Control(kind) => node.measure_control(kind, axis, for_size),
    NodeKind::Text => {
      let Some(cell) = node.text.borrow().clone() else {
        return Measurement::default();
      };
      let measured = match axis {
        Axis::Horizontal => {
          let intrinsic = cell.intrinsic();

          return Measurement {
            minimum: intrinsic.min,
            natural: intrinsic.max,
            baseline: None,
          };
        }
        Axis::Vertical => cell.measure(
          (for_size, None),
          (
            for_size.map_or(Space::MaxContent, Space::Definite),
            Space::MaxContent,
          ),
        ),
      };

      Measurement {
        minimum: measured.height,
        natural: measured.height,
        baseline: measured.first_baseline,
      }
    }
    NodeKind::Painted
    | NodeKind::Image
    | NodeKind::Native
    | NodeKind::Scope
    | NodeKind::Segment => Measurement::default(),
  }
}

impl LayoutHost for Host {
  type Child = Node;

  fn measure(child: &Node, axis: Axis, for_size: Option<f32>) -> Measurement {
    let epoch = EPOCH.with(Cell::get);
    let key = for_size.map(f32::to_bits);

    {
      let mut cache = child.measures.borrow_mut();

      if cache.0 != epoch {
        *cache = (epoch, Vec::new());
      }

      if let Some((_, _, measured)) = cache
        .1
        .iter()
        .find(|(cached, size, _)| *cached == axis && *size == key)
      {
        return *measured;
      }
    }

    MEASURED.with(|count| count.set(count.get() + 1));

    let measured = measure_uncached(child, axis, for_size);

    child.measures.borrow_mut().1.push((axis, key, measured));

    measured
  }

  fn constant_height(child: &Node) -> bool {
    matches!(child.spec.kind, NodeKind::Control(kind) if kind != skene_foundation::seam::node::ControlKind::TextEditor)
  }

  fn is_visible(child: &Node) -> bool {
    !child.is_hidden()
  }

  fn is_container(child: &Node) -> bool {
    child.is_container()
  }

  fn text(child: &Node) -> Option<Rc<dyn MeasureText>> {
    match child.spec.kind {
      NodeKind::Text => child
        .text
        .borrow()
        .clone()
        .map(|cell| cell as Rc<dyn MeasureText>),
      _ => None,
    }
  }

  fn allocate(child: &Node, rect: Rect, fractional: Rect, _: Option<f32>) {
    // Whole pixels relative to the container, rounded in the window as one tree would be;
    // the fractional rectangle (in the window) lays a nested container's children out.
    child.frame.set(rect);
    child.laid_width.set(fractional.width);
    lay_children(child, fractional);
  }
}

/// Lays `node`'s children out inside `window`, its rectangle in the window.
fn lay_children(node: &Node, window: Rect) {
  match node.spec.kind {
    NodeKind::Layout(LayoutKind::Scroll {
      horizontal,
      vertical,
    }) => {
      let Some(child) = scrolled(node) else {
        return;
      };
      let width = if horizontal {
        Host::measure(&child, Axis::Horizontal, None)
          .natural
          .max(window.width)
      } else {
        window.width
      };
      let height = if vertical {
        Host::measure(&child, Axis::Vertical, Some(width.round()))
          .natural
          .max(window.height)
      } else {
        window.height
      };
      let (sx, sy) = node.scroll.get();
      let clamped = (
        sx.clamp(0., (width - window.width).max(0.)),
        sy.clamp(0., (height - window.height).max(0.)),
      );

      node.scroll.set(clamped);
      node.extent.set((width, height));
      child.laid_width.set(width);
      child.frame.set(Rect {
        x: 0.,
        y: 0.,
        width,
        height,
      });
      lay_children(
        &child,
        Rect {
          x: window.x - clamped.0,
          y: window.y - clamped.1,
          width,
          height,
        },
      );
    }
    NodeKind::Layout(_) => {
      with_engine(node, |engine| {
        engine.allocate((window.width, window.height), Some(window));
      });
    }
    _ => {}
  }
}

/// Lays the tree under `root` out in a window `width` by `height`, the root at `origin`.
pub fn layout(root: &Node, origin: (f32, f32), width: f32, height: f32) {
  next_epoch();

  let frame = Rect {
    x: origin.0,
    y: origin.1,
    width,
    height,
  };

  root.frame.set(frame);
  lay_children(root, frame);
}

/// Computes what each node under `root` and its subtree paint, relative to where it is drawn
/// (its frame moved by its offset): its own box and overhang, and its children's where it does
/// not clip them.
pub fn reach(root: &Node) {
  fn go(node: &Node) -> Rect {
    let frame = node.frame.get();
    let own = crate::paint::painted_bounds(
      node,
      Rect {
        x: 0.,
        y: 0.,
        width: frame.width,
        height: frame.height,
      },
    );
    let clips =
      node.look.borrow().clip || matches!(node.layout_kind(), Some(LayoutKind::Scroll { .. }));
    let (sx, sy) = node.scroll.get();
    let mut reach = own;

    for child in node.children() {
      if child.is_hidden() {
        continue;
      }

      let child_frame = child.frame.get();
      let (dx, dy) = child.offset();
      let child_reach = go(&child);

      if !clips {
        reach = crate::paint::union(
          reach,
          Rect {
            x: child_reach.x + child_frame.x + dx - sx,
            y: child_reach.y + child_frame.y + dy - sy,
            ..child_reach
          },
        );
      }
    }

    node.reach.set(reach);

    reach
  }

  go(root);
}

/// Gives every paragraph under `root` its text in the run properties it inherits, rebuilding
/// only those whose content or metrics changed: the strongest change made.
pub fn prepare(root: &Node, palette: &Palette) -> Change {
  fn go(node: &Node, inherited: &RunProps, palette: &Palette, change: &mut Change) {
    if !node.is_alive() {
      return;
    }

    let props = node.run.borrow().over(inherited);

    node.ink.set(props.color.unwrap_or(palette.ink));

    let built: Option<(AttributedText, _, Vec<_>)> = match node.spec.kind {
      NodeKind::Text => {
        let contents = node.contents.borrow();
        let base = props.run_style(palette.ink, DEFAULT_SIZE);
        let paragraph = props.paragraph_style();

        match (&contents.runs, &contents.text) {
          (Some(runs), _) => {
            let (text, links) = attributed_runs(runs, &props, &base, palette.ink, DEFAULT_SIZE);

            Some((text, paragraph, links))
          }
          (None, Some(text)) => Some((
            AttributedText::plain(Rc::clone(text), base),
            paragraph,
            Vec::new(),
          )),
          (None, None) => Some((AttributedText::plain("", base), paragraph, Vec::new())),
        }
      }
      NodeKind::Control(kind) => {
        if controls::is_entry(kind) && node.prepare_entry(&props, palette) {
          *change = Change::Layout;
        }

        node
          .control_label(kind, &props, palette)
          .map(|(text, paragraph)| (text, paragraph, Vec::new()))
      }
      _ => None,
    };

    match built {
      Some((text, paragraph, links)) => {
        let cell = node
          .text
          .borrow_mut()
          .get_or_insert_with(TextCell::new)
          .clone();
        let made = cell.set(text, paragraph);

        cell.set_links(links);

        *change = match (*change, made) {
          (Change::Layout, _) | (_, Change::Layout) => Change::Layout,
          (Change::Paint, _) | (_, Change::Paint) => Change::Paint,
          _ => Change::None,
        };
      }
      None => {
        if matches!(node.spec.kind, NodeKind::Control(_)) && node.text.borrow_mut().take().is_some()
        {
          *change = Change::Layout;
        }
      }
    }

    for child in node.children() {
      go(&child, &props, palette, change);
    }
  }

  let mut change = Change::None;

  go(root, &RunProps::default(), palette, &mut change);

  change
}
