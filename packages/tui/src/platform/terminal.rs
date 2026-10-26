//! The terminal platform: [`Terminal`], its escape trait and its seams beyond the
//! [`Realization`](skene_foundation::Realization) trait, realized by
//! `skene-tui-terminal`.
//!
//! For a library author writing a view directly against the terminal's cells. A
//! low-level view, a `View<Active>` like any other, paints through [`CellPainter`] with
//! [`Cells`], the handle addressing its box's cells from their own origin, and the
//! module exists in the terminal's build alone, so cell-reaching code compiles for the
//! terminal alone. The terminal has no toolkit's view to adopt, so its native view is
//! [`Unadoptable`] and the native door cannot be reached there. Its text engine is the
//! [`CellEngine`], which every text lays out through and its drawn entries edit over.
//!
//! The terminal implements [`Realization`](skene_foundation::Realization) for
//! [`Terminal`] and exposes its own entry points beside it: [`install`], [`mount`] and
//! [`uninstall`] for an in-process test, and [`adopt_restoration`] and
//! [`dev_notice_view`] for a dev reload's restore and its rebuild notice. The facade's
//! root keeps [`launch`](crate::launch) and its [`Launch`](crate::Launch), which install
//! and mount for a running app.

pub use skene_tui_terminal::{
  CellEngine,
  CellPainter,
  Cells,
  StandIns,
  Terminal,
  Unadoptable,
  adopt_restoration,
  dev_notice_view,
  install,
  mount,
  uninstall,
};
