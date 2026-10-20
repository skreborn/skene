//! Layout: the foundation's engine over the realized tree. [`Host`] answers the engine's host
//! protocol for every realized node (a container through its own engine, a text through its
//! paragraph, a text entry through its editor), a scroller lays its one child out at the
//! height it asks for and its bars along its edges, an overlay is out of its parent's flow and
//! laid out apart
//! ([`Drawn::layout_overlay`]), and [`prepare`] gives every paragraph the run properties it inherits
//! before a layout reads it.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{
  layout::{Axis, ContainerLayout, LayoutHost, Measurement, Needed, Rect, ResolvedLayout},
  seam::node::{LayoutKind, NodeKind},
  text::{AttributedText, MeasureText, ParagraphStyle, Space, TextEngine as _},
  value::{Dimension, GridTrack, Inset, Tracks},
};

use crate::{
  look::RunProps,
  node::{Drawn, Measures, Node, Rigid},
  palette::{DEFAULT_SIZE, Palette},
  text::{Change, Links, TextCell, attributed_runs},
};

thread_local! {
  /// The scroll boxes whose offsets a layout clamped to their extents, to report.
  static CLAMPED: std::cell::RefCell<Vec<Node>> = const { std::cell::RefCell::new(Vec::new()) };
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

/// The scroll boxes whose offsets the layouts since the last take clamped to their extents.
pub fn take_clamped() -> Vec<Node> {
  CLAMPED.with(|clamped| std::mem::take(&mut *clamped.borrow_mut()))
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
  let epoch = EPOCH.with(Cell::get);
  let mut slot = node.engine.borrow_mut();

  if node.engine_epoch.replace(epoch) != epoch || slot.is_none() {
    let children: Vec<(Node, ResolvedLayout)> = node
      .children_ref()
      .iter()
      .map(|child| (Rc::clone(child), child.layout.borrow().clone()))
      .collect();
    let mut own = own_layout(node);

    if node.parent().is_none() && !node.is_overlay() {
      // The window's root holds the app's view in the window's own cell, as a page's first
      // box fills the viewport: never widened or lengthened to its content, so nothing is
      // measured to place it.
      let whole = Some(Tracks::from([GridTrack::Fraction(1.)]));

      own.columns.clone_from(&whole);
      own.rows = whole;
    }

    match &mut *slot {
      Some(engine) => engine.set(own, children),
      None => {
        let kind = node.layout_kind().unwrap_or(LayoutKind::Box);

        *slot = Some(ContainerLayout::with_children(kind, own, children));
      }
    }
  }

  match &mut *slot {
    Some(engine) => body(engine),
    None => unreachable!("the engine is made above"),
  }
}

/// The one child a scroller scrolls: its first shown child that is not a bar.
fn scrolled(node: &Node) -> Option<Node> {
  node
    .children_ref()
    .iter()
    .find(|child| !child.is_hidden() && child.bar_axis().is_none())
    .cloned()
}

/// The length along `axis` a node's own least size along it names inside a length of
/// `within`: none where it names none.
fn least(layout: &ResolvedLayout, axis: Axis, within: f32) -> f32 {
  let minimum = match axis {
    Axis::Horizontal => layout.min_width,
    Axis::Vertical => layout.min_height,
  };

  match minimum {
    Some(Dimension::Length(length)) => length,
    Some(Dimension::Fraction(fraction)) => fraction * within,
    Some(Dimension::Auto) | None => 0.,
  }
}

/// Lays the scroll box `node`'s bars out over its viewport `window` (its rectangle in the
/// window), unscrolled: a bar along each axis its content overflows shown along the trailing or
/// bottom edge, as thick as its track asks, the vertical one stopping short of the horizontal
/// one; every other bar hidden. Each thumb is as long as its track's inner length times the
/// share of the content in view, its least size along the axis a floor.
fn lay_bars(node: &Node, window: Rect) {
  let (width, height) = node.extent.get();
  let bars: Vec<(Node, Axis)> = node
    .children_ref()
    .iter()
    .filter_map(|child| child.bar_axis().map(|axis| (Rc::clone(child), axis)))
    .collect();
  let across = |axis: Axis| match axis {
    Axis::Horizontal => Axis::Vertical,
    Axis::Vertical => Axis::Horizontal,
  };
  let overflows = |axis: Axis| match axis {
    Axis::Horizontal => width > window.width + 0.5,
    Axis::Vertical => height > window.height + 0.5,
  };
  let thickness = |track: &Node, axis: Axis| {
    Host::measure(track, across(axis), None, Needed::Natural)
      .natural
      .round()
  };
  let shown = |wanted: Axis| {
    bars
      .iter()
      .find(|(_, axis)| *axis == wanted && overflows(wanted))
      .map_or(0., |(track, axis)| thickness(track, *axis))
  };

  for (track, axis) in &bars {
    if !overflows(*axis) {
      track.set_hidden(true);

      continue;
    }

    track.set_hidden(false);

    let thick = thickness(track, *axis);
    let rect = match axis {
      Axis::Vertical => Rect {
        x: window.width - thick,
        y: 0.,
        width: thick,
        height: (window.height - shown(Axis::Horizontal)).max(0.),
      },
      Axis::Horizontal => Rect {
        x: 0.,
        y: window.height - thick,
        width: (window.width - shown(Axis::Vertical)).max(0.),
        height: thick,
      },
    };
    let placed = Rect {
      x: window.x + rect.x,
      y: window.y + rect.y,
      ..rect
    };

    track.frame.set(rect);
    track.laid_width.set(rect.width);
    lay_children(track, placed);

    let Some(thumb) = track.children_ref().first().cloned() else {
      continue;
    };
    let padding = own_layout(track).padding.unwrap_or(Inset {
      top: 0.,
      trailing: 0.,
      bottom: 0.,
      leading: 0.,
    });
    let (start, inner, share) = match axis {
      Axis::Vertical => (
        padding.top,
        rect.height - padding.top - padding.bottom,
        window.height / height,
      ),
      Axis::Horizontal => (
        padding.leading,
        rect.width - padding.leading - padding.trailing,
        window.width / width,
      ),
    };
    let inner = inner.max(0.);
    let length = (inner * share)
      .max(least(&thumb.layout.borrow(), *axis, inner))
      .min(inner)
      .round();
    let laid = thumb.frame.get();
    let frame = match axis {
      Axis::Vertical => Rect {
        y: start,
        height: length,
        ..laid
      },
      Axis::Horizontal => Rect {
        x: start,
        width: length,
        ..laid
      },
    };

    thumb.frame.set(frame);
    thumb.laid_width.set(frame.width);
    thumb.thumb.set(Some((*axis, inner - length)));
    lay_children(
      &thumb,
      Rect {
        x: placed.x + frame.x,
        y: placed.y + frame.y,
        ..frame
      },
    );
  }
}

impl Drawn {
  /// Its measurement along `axis`, the other axis `for_size` where known, of what `needed`
  /// names, its cache left aside.
  fn measure_uncached(
    self: &Rc<Self>,
    axis: Axis,
    for_size: Option<f32>,
    needed: Needed,
  ) -> Measurement {
    match self.spec.kind {
      NodeKind::Layout(LayoutKind::Scroll {
        horizontal,
        vertical,
      }) => {
        let Some(child) = scrolled(self) else {
          return Measurement::default();
        };
        let scrolls = match axis {
          Axis::Horizontal => horizontal,
          Axis::Vertical => vertical,
        };

        if !scrolls {
          return Host::measure(&child, axis, for_size, needed);
        }

        Measurement {
          minimum: 0.,
          natural: if needed.natural() {
            Host::measure(&child, axis, for_size, Needed::Natural).natural
          } else {
            0.
          },
          baseline: None,
        }
      }
      NodeKind::Layout(_) | NodeKind::Overlay => {
        with_engine(self, |engine| engine.measure(axis, for_size, needed))
      }
      NodeKind::Editable { multiline } => self.measure_entry(multiline, axis, for_size),
      NodeKind::Text => {
        let Some(cell) = self.text.borrow().clone() else {
          return Measurement::default();
        };
        let measured = match axis {
          Axis::Horizontal => {
            let intrinsic = cell.intrinsic();

            return Measurement {
              minimum: intrinsic.min,
              natural: intrinsic.max.max(cell.reserved_width()),
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
}

impl Drawn {
  /// Which of its sizes do not depend on the other axis's, found once a layout epoch: a
  /// text's, an entry's and a leaf's never do; a vertical scroller's least height is none; a
  /// container's are rigid where its shown children's are, a column's height where none of
  /// them takes a basis of its own, and a wrapping column's width never.
  fn rigid(self: &Rc<Self>) -> Rigid {
    let epoch = EPOCH.with(Cell::get);

    {
      let cache = self.measures.borrow();

      if cache.epoch == epoch
        && let Some(rigid) = cache.rigid
      {
        return rigid;
      }
    }

    let all = Rigid {
      height: true,
      width: true,
    };
    let found = match self.spec.kind {
      NodeKind::Layout(LayoutKind::Scroll { vertical, .. }) => {
        scrolled(self).map_or(all, |child| {
          let inner = child.rigid();

          Rigid {
            height: !vertical && inner.height,
            width: inner.width,
          }
        })
      }
      NodeKind::Layout(_) | NodeKind::Overlay => {
        let column = self.layout_kind() == Some(LayoutKind::Column);
        let wraps = self.layout.borrow().wrap;

        self
          .children_ref()
          .iter()
          .filter(|child| Host::is_visible(child))
          .fold(
            Rigid {
              height: true,
              width: !(column && wraps),
            },
            |found, child| {
              let inner = child.rigid();
              let basis = column
                && child
                  .layout
                  .borrow()
                  .flexibility
                  .is_some_and(|flexibility| flexibility.initial != Dimension::Auto);

              Rigid {
                height: found.height && inner.height && !basis,
                width: found.width && inner.width,
              }
            },
          )
      }
      _ => all,
    };
    let mut cache = self.measures.borrow_mut();

    if cache.epoch != epoch {
      *cache = Measures {
        epoch,
        ..Measures::default()
      };
    }

    cache.rigid = Some(found);

    found
  }
}

/// A node's measurement along `axis` with what it holds: `kept`, holding what `had` names,
/// joined with `fresh`, holding what `got` names, measured together.
const fn joined(
  axis: Axis,
  kept: Measurement,
  had: Needed,
  fresh: Measurement,
  got: Needed,
) -> Measurement {
  let minimum = if had.minimum() {
    kept.minimum
  } else {
    fresh.minimum
  };
  let (natural, baseline) = if had.natural() {
    (kept.natural, kept.baseline)
  } else {
    (fresh.natural, fresh.baseline)
  };

  if !(had.minimum() || got.minimum()) || !(had.natural() || got.natural()) {
    return Measurement {
      minimum,
      natural,
      baseline,
    };
  }

  match axis {
    Axis::Horizontal => Measurement {
      minimum,
      natural: natural.max(minimum),
      baseline,
    },
    Axis::Vertical => Measurement {
      minimum: minimum.min(natural),
      natural,
      baseline,
    },
  }
}

/// What `a` and `b` name together.
const fn both(a: Needed, b: Needed) -> Needed {
  match (a.minimum() || b.minimum(), a.natural() || b.natural()) {
    (true, true) => Needed::Both,
    (true, false) => Needed::Minimum,
    _ => Needed::Natural,
  }
}

/// What `needed` names that `had` does not, where anything.
const fn missing(needed: Needed, had: Needed) -> Option<Needed> {
  match (
    needed.minimum() && !had.minimum(),
    needed.natural() && !had.natural(),
  ) {
    (true, true) => Some(Needed::Both),
    (true, false) => Some(Needed::Minimum),
    (false, true) => Some(Needed::Natural),
    (false, false) => None,
  }
}

impl LayoutHost for Host {
  type Child = Node;

  fn measure(child: &Node, axis: Axis, for_size: Option<f32>, needed: Needed) -> Measurement {
    let rigid = child.rigid();
    // A width that does not depend on the height is measured once, whatever height is given;
    // a height whose least is its natural one is measured once for both.
    let (for_size, needed) = match axis {
      Axis::Horizontal if rigid.width => (None, needed),
      Axis::Vertical if rigid.height => (for_size, Needed::Both),
      _ => (for_size, needed),
    };
    let epoch = EPOCH.with(Cell::get);
    let key = for_size.map(f32::to_bits);
    let kept = {
      let mut cache = child.measures.borrow_mut();

      if cache.epoch != epoch {
        *cache = Measures {
          epoch,
          ..Measures::default()
        };
      }

      cache
        .kept
        .iter()
        .find(|(cached, size, _, _)| *cached == axis && *size == key)
        .map(|(_, _, measured, had)| (*measured, *had))
    };
    let (wanted, kept) = match kept {
      Some((measured, had)) => match missing(needed, had) {
        None => return measured,
        Some(wanted) => (wanted, Some((measured, had))),
      },
      None => (needed, None),
    };

    MEASURED.with(|count| count.set(count.get() + 1));

    let fresh = if axis == Axis::Vertical && rigid.height {
      let natural = child.measure_uncached(axis, for_size, Needed::Natural);

      debug_assert_eq!(
        child
          .measure_uncached(axis, for_size, Needed::Minimum)
          .minimum,
        natural.natural,
        "a rigid node's least height is its natural height"
      );

      Measurement {
        minimum: natural.natural,
        ..natural
      }
    } else {
      child.measure_uncached(axis, for_size, wanted)
    };
    let (measured, holds) = match kept {
      Some((kept, had)) => (joined(axis, kept, had, fresh, wanted), both(had, wanted)),
      None => (fresh, wanted),
    };
    let mut cache = child.measures.borrow_mut();

    match cache
      .kept
      .iter_mut()
      .find(|(cached, size, _, _)| *cached == axis && *size == key)
    {
      Some(entry) => {
        entry.2 = measured;
        entry.3 = holds;
      }
      None => cache.kept.push((axis, key, measured, holds)),
    }

    measured
  }

  fn constant_height(child: &Node) -> bool {
    child.spec.kind == NodeKind::Editable { multiline: false }
  }

  fn is_visible(child: &Node) -> bool {
    !child.is_hidden() && !child.is_overlay()
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
        node.extent.set((window.width, window.height));
        lay_bars(node, window);

        return;
      };
      let width = if horizontal {
        Host::measure(&child, Axis::Horizontal, None, Needed::Natural)
          .natural
          .max(window.width)
      } else {
        window.width
      };
      let height = if vertical {
        Host::measure(&child, Axis::Vertical, Some(width.round()), Needed::Natural)
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

      if clamped != (sx, sy) {
        CLAMPED.with(|list| list.borrow_mut().push(Rc::clone(node)));
      }

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
      lay_bars(node, window);
    }
    NodeKind::Layout(_) | NodeKind::Overlay => {
      with_engine(node, |engine| {
        engine.allocate((window.width, window.height), Some(window));
      });
    }
    _ => {}
  }
}

impl Drawn {
  /// Lays its content out at `window`, its rectangle in the window, where it is an overlay.
  pub fn layout_overlay(self: &Rc<Self>, window: Rect) {
    self.frame.set(window);
    self.laid_width.set(window.width);
    lay_children(self, window);
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
  fn go(node: &Node) {
    for child in node.children_ref().iter() {
      if !child.is_hidden() && !child.is_overlay() {
        go(child);
      }
    }

    node.gather_reach();
  }

  go(root);
}

impl Drawn {
  /// Computes what it and its subtree paint ([`reach`]) from its own box and its children's
  /// reach as last computed.
  fn gather_reach(self: &Rc<Self>) {
    let frame = self.frame.get();
    let own = crate::paint::painted_bounds(
      self,
      Rect {
        x: 0.,
        y: 0.,
        width: frame.width,
        height: frame.height,
      },
    );
    let clips =
      self.look.borrow().clip || matches!(self.layout_kind(), Some(LayoutKind::Scroll { .. }));
    let mut reach = own;

    if !clips {
      for child in self.children_ref().iter() {
        if child.is_hidden() || child.is_overlay() {
          continue;
        }

        let (sx, sy) = self.scroll_for(child);
        let child_frame = child.frame.get();
        let (dx, dy) = child.offset();
        let child_reach = child.reach.get();

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

    self.reach.set(reach);
  }

  /// Computes again what it and each of its ancestors paint, its children's reach kept: what a
  /// change of its look alone (its fill, its shadow, its ring, its offset) needs.
  pub fn refresh_reach(self: &Rc<Self>) {
    self.gather_reach();

    for ancestor in self.ancestors() {
      ancestor.gather_reach();
    }
  }
}

/// Gives every paragraph under `root` its text in the run properties it inherits, rebuilding
/// only those whose content or metrics changed: the strongest change made, the nodes whose
/// paint alone changed pushed to `repainted`.
pub fn prepare(root: &Node, palette: &Palette, repainted: &mut Vec<Node>) -> Change {
  prepare_from(root, &RunProps::default(), palette, repainted)
}

impl Drawn {
  /// [`prepare`] for its subtree alone, the run properties its ancestors set inherited:
  /// what a node whose own run properties changed, and nothing else, needs.
  pub fn prepare_subtree(self: &Rc<Self>, palette: &Palette, repainted: &mut Vec<Node>) -> Change {
    let mut ancestors = self.ancestors();

    ancestors.reverse();

    let inherited = ancestors
      .iter()
      .fold(RunProps::default(), |inherited, ancestor| {
        ancestor.run.borrow().over(&inherited)
      });

    prepare_from(self, &inherited, palette, repainted)
  }
}

fn prepare_from(
  root: &Node,
  inherited: &RunProps,
  palette: &Palette,
  repainted: &mut Vec<Node>,
) -> Change {
  /// Paragraphs to make: each node, its cell and its link runs, and beside them the texts
  /// and styles to make them of.
  type Pending = (
    Vec<(Node, Rc<TextCell>, Links)>,
    Vec<(AttributedText, ParagraphStyle)>,
  );

  /// What `made` changed in `node`'s cell, once it shows its text: its reserve and links
  /// set, the change it makes recorded.
  fn settle(
    node: &Node,
    cell: &TextCell,
    mut made: Change,
    links: Links,
    change: &mut Change,
    repainted: &mut Vec<Node>,
  ) {
    let reserve = node.contents.borrow().reserve.clone();
    let reserve_moved = cell.set_reserve(reserve, made == Change::Layout);

    cell.set_links(links);

    if made == Change::Layout && !reserve_moved && cell.keeps_its_size() {
      // One line as wide as the widest text it reserves: its size does not move.
      made = Change::Paint;
    }

    if reserve_moved {
      made = Change::Layout;
    }

    match made {
      Change::Layout => *change = Change::Layout,
      Change::Paint => repainted.push(Rc::clone(node)),
      Change::None => {}
    }
  }

  fn go(
    node: &Node,
    inherited: &RunProps,
    palette: &Palette,
    change: &mut Change,
    repainted: &mut Vec<Node>,
    pending: &mut Pending,
  ) {
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
      NodeKind::Editable { .. } => {
        if node.prepare_entry(&props, palette) {
          *change = Change::Layout;
        }

        let placeholder = node.contents.borrow().placeholder.clone();

        placeholder.map(|placeholder| {
          let ink = props.placeholder_color.unwrap_or(palette.dim);
          let mut style = props.run_style(ink, DEFAULT_SIZE);

          style.paint.color = ink;

          (
            AttributedText::plain(placeholder, style),
            props
              .paragraph_style()
              .with_wrap(skene_foundation::text::Wrap::None),
            Vec::new(),
          )
        })
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

        match cell.update(text, paragraph) {
          Ok(made) => settle(node, &cell, made, links, change, repainted),
          Err(made) => {
            pending.0.push((Rc::clone(node), cell, links));
            pending.1.push(made);
          }
        }
      }
      None => {
        if node.is_entry() && node.text.borrow_mut().take().is_some() {
          repainted.push(Rc::clone(node));
        }
      }
    }

    for child in node.children_ref().iter() {
      go(child, &props, palette, change, repainted, pending);
    }
  }

  let mut change = Change::None;
  let mut pending = (Vec::new(), Vec::new());

  go(
    root,
    inherited,
    palette,
    &mut change,
    repainted,
    &mut pending,
  );

  // Every paragraph to make is made at once, so an engine may make them side by side.
  let (cells, texts) = pending;
  let paragraphs = crate::text::engine().paragraphs(&texts);

  for ((node, cell, links), ((text, style), paragraph)) in
    cells.into_iter().zip(texts.into_iter().zip(paragraphs))
  {
    cell.install(text, style, paragraph);
    settle(&node, &cell, Change::Layout, links, &mut change, repainted);
  }

  change
}
