//! The portable event vocabulary: what a handler receives, with no platform types—and
//! the dispatch contract every facade honors.
//!
//! Dispatch is deterministic and identical on every facade. For each event, the user
//! handlers run first, in a fixed order: deepest-to-shallowest along the hit path for
//! pointer events and clicks, the focused node then its ancestors for keys. Every
//! handler on one node always runs; bubbling is between nodes. After the handlers, the
//! platform's own response runs—the browser's default action, the terminal's focus
//! movement on `Tab` and activation of the control it realizes (a toggle, check, cycle,
//! or jump), input editing, `Escape` dismissal—unless a handler prevented it.
//!
//! A handler's closure returns its verdict as anything `Into<Prevent>`: `()` to
//! observe, or a [`Prevent`] to suppress the platform's response, stop bubbling, or
//! both; prevention only accumulates. Notification events—change, toggle, scroll
//! offsets, focus and blur—report what already happened and take no verdict, and a
//! terminal's `Ctrl-C` stays unconditional.
//!
//! A view that composes its own behavior out of these events—every control that draws
//! itself—is not the platform, and [`Prevent`] does not reach it. Such a view takes its
//! user's handler as an [`Override`], which runs ahead of the behavior and may take the
//! event outright.

use std::{ops::ControlFlow, rc::Rc};

/// What a handler prevents of the dispatch that invoked it.
///
/// A handler returns anything `Into<Prevent>`; `()` converts to
/// [`None`](Prevent::None), so an observing handler needs no return at all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum Prevent {
  /// Prevents nothing: the platform's response runs and bubbling continues.
  #[default]
  None,
  /// Suppresses the platform's own response; shallower handlers still run.
  Platform,
  /// Skips the shallower handlers; the platform's response still runs.
  Bubbling,
  /// Both: no platform response, no shallower handlers.
  All,
}

impl From<()> for Prevent {
  #[inline(always)]
  fn from((): ()) -> Self {
    Self::None
  }
}

/// What one dispatch has had prevented so far.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub struct Prevented {
  /// Whether the platform's own response is suppressed.
  pub platform: bool,
  /// Whether bubbling stops after the current node.
  pub bubbling: bool,
}

impl Prevented {
  /// Folds one handler's verdict in; prevention only accumulates.
  pub fn absorb(&mut self, verdict: Prevent) {
    match verdict {
      Prevent::None => {}
      Prevent::Platform => self.platform = true,
      Prevent::Bubbling => self.bubbling = true,
      Prevent::All => {
        self.platform = true;
        self.bubbling = true;
      }
    }
  }

  /// Folds another accumulated prevention in.
  pub fn merge(&mut self, other: Self) {
    self.platform |= other.platform;
    self.bubbling |= other.bubbling;
  }
}

/// A handler a view's user installs ahead of the view's own behavior.
///
/// A view that composes behavior out of the event vocabulary—a control that draws
/// itself—holds one of these per event it acts on, and registers what
/// [`wrap`](Self::wrap) returns. The installed handler runs first:
/// [`Continue`](ControlFlow::Continue) leaves the view to act as though nothing were
/// installed, and [`Break`](ControlFlow::Break) takes the event, carrying the verdict
/// the dispatch goes on to see.
///
/// `E` is the event the handler receives, or `()` for an event with no payload.
#[must_use]
pub struct Override<E>(Option<OverrideHandler<E>>);

/// The handler an [`Override`] holds, as it stores it.
type OverrideHandler<E> = Rc<dyn Fn(&E) -> ControlFlow<Prevent>>;

impl<E> Override<E> {
  /// Creates an override with no handler installed.
  #[inline(always)]
  pub const fn new() -> Self {
    Self(None)
  }

  /// Installs `handler`, replacing any already installed.
  #[inline(always)]
  pub fn set(&mut self, handler: impl Fn(&E) -> ControlFlow<Prevent> + 'static) {
    self.0 = Some(Rc::new(handler));
  }

  /// Folds the installed handler and the view's own `behavior` into the single handler
  /// the view registers on its node.
  pub fn wrap(self, behavior: impl Fn(&E) -> Prevent + 'static) -> impl Fn(&E) -> Prevent {
    move |event| match &self.0 {
      Some(handler) => match handler(event) {
        ControlFlow::Continue(()) => behavior(event),
        ControlFlow::Break(prevent) => prevent,
      },
      None => behavior(event),
    }
  }
}

impl<E> Clone for Override<E> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<E> Default for Override<E> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

/// A click handler as dispatch stores it: the closure plus its verdict.
pub type ClickHandler = dyn Fn() -> Prevent;
/// A key handler as dispatch stores it: the closure plus its verdict.
pub type KeyHandler = dyn Fn(&KeyEvent) -> Prevent;
/// A pointer handler as dispatch stores it: the closure plus its verdict.
pub type PointerHandler = dyn Fn(&PointerEvent) -> Prevent;

/// The modifier-key state accompanying an input event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
// One bool per modifier key is the domain shape.
#[expect(clippy::struct_excessive_bools)]
pub struct Modifiers {
  /// Whether Control was held.
  pub ctrl: bool,
  /// Whether Shift was held.
  pub shift: bool,
  /// Whether Alt was held.
  pub alt: bool,
  /// Whether the platform command/meta key was held.
  pub meta: bool,
}

/// A keyboard event: the logical key plus modifier state.
///
/// `key` follows the W3C key-value names (`"a"`, `"Enter"`, `"ArrowLeft"`, …); every
/// driver maps its platform's keys onto them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct KeyEvent {
  /// The logical key value.
  pub key: String,
  /// The modifier state.
  pub modifiers: Modifiers,
}

/// A pointer button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum PointerButton {
  /// The primary button (usually the left).
  Primary,
  /// The secondary button (usually the right).
  Secondary,
  /// The auxiliary button (usually the wheel).
  Auxiliary,
}

/// A pointer event: where, which button, and the modifier state.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct PointerEvent {
  /// The pointer's horizontal position, in viewport logical pixels.
  pub x: f32,
  /// The pointer's vertical position, in viewport logical pixels.
  pub y: f32,
  /// The pointer's horizontal position, relative to the handling view's top-left
  /// corner.
  pub local_x: f32,
  /// The pointer's vertical position, relative to the handling view's top-left corner.
  pub local_y: f32,
  /// The button that changed state, or `None` for a move.
  pub button: Option<PointerButton>,
  /// The modifier state.
  pub modifiers: Modifiers,
}

/// A scroll: the scrolled container's new offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct ScrollEvent {
  /// The horizontal scroll offset, in logical pixels.
  pub x: f32,
  /// The vertical scroll offset, in logical pixels.
  pub y: f32,
}
