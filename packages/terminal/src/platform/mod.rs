//! The terminal's own vocabulary, in [`terminal`], and [`Platform`], the build's
//! platform as a const.
//!
//! A platform's module holds what only that platform has, in its own build: the terminal
//! is this facade's one platform, so what [`terminal`] holds is there in every build
//! the facade compiles in. The foundation names no platform: everything here belongs to
//! the facade.
//!
//! ```
//! use skene_terminal::Platform;
//!
//! const NAME: &str = Platform::CURRENT.name();
//!
//! assert_eq!(NAME, "terminal");
//! ```

pub mod terminal;

/// A terminal platform: what a build of this facade names by
/// `--cfg skene_platform="<name>"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Platform {
  /// A terminal, through its device.
  Terminal,
}

impl Platform {
  /// The platform the build names.
  pub const CURRENT: Self = Self::Terminal;

  /// The platform's name, as its build names it in `skene_platform`.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Terminal => "terminal",
    }
  }
}
