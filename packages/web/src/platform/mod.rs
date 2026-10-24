//! Each pixel platform's own vocabulary, a module per platform, and [`Platform`], the
//! build's platform as a const.
//!
//! A platform's module holds what only that platform has: its data types exist in every
//! build, so code naming one compiles wherever the facade does, and its controls exist
//! in its own build alone, so code naming one is written inside an arm that matches its
//! platform, under `#[cfg(skene_platform = "<p>")]`. [`apple`] is the one grouping, for
//! what iOS and macOS share. The foundation names no platform: everything here belongs
//! to the facade.
//!
//! An app that diverges per platform matches on [`Platform::CURRENT`], a const the
//! compiler folds, so the arms of every other platform cost nothing.
//!
//! ```
//! use skene_web::Platform;
//!
//! let greeting = match Platform::CURRENT {
//!   Platform::Ios | Platform::Macos => "Hello from Apple",
//!   Platform::Web if Platform::IS_SERVER => "Hello from the server",
//!   platform => platform.name(),
//! };
//!
//! assert!(!greeting.is_empty());
//! ```

pub mod web;

/// Android's own vocabulary: its data types in every build, its controls in the
/// `android` build alone. It holds nothing yet.
pub mod android {}

/// What iOS and macOS share ([`Platform::IS_APPLE`]): its data types in every build, its
/// controls in the `ios` and `macos` builds alone. It holds nothing yet.
pub mod apple {}

/// iOS's own vocabulary: its data types in every build, its controls in the `ios` build
/// alone. It holds nothing yet.
pub mod ios {}

/// Linux's own vocabulary: its data types in every build, its controls in the `linux`
/// build alone. It holds nothing yet.
pub mod linux {}

/// macOS's own vocabulary: its data types in every build, its controls in the `macos`
/// build alone. It holds nothing yet.
pub mod macos {}

/// Windows's own vocabulary: its data types in every build, its controls in the
/// `windows` build alone. It holds nothing yet.
pub mod windows {}

/// A pixel platform: what a build of this facade names by
/// `--cfg skene_platform="<name>"`.
///
/// The web's two leaves are one platform: the server leaf is the web build with
/// `--cfg skene_server` beside it, which [`IS_SERVER`](Self::IS_SERVER) answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Platform {
  /// The web: the browser, and the server leaf that renders its pages.
  Web,
  /// iOS.
  Ios,
  /// Android.
  Android,
  /// macOS.
  Macos,
  /// Windows.
  Windows,
  /// Linux.
  Linux,
}

impl Platform {
  /// The platform the build names.
  pub const CURRENT: Self = if cfg!(skene_platform = "ios") {
    Self::Ios
  }
  else if cfg!(skene_platform = "android") {
    Self::Android
  }
  else if cfg!(skene_platform = "macos") {
    Self::Macos
  }
  else if cfg!(skene_platform = "windows") {
    Self::Windows
  }
  else if cfg!(skene_platform = "linux") {
    Self::Linux
  }
  else {
    Self::Web
  };

  /// Whether the build is the web's server leaf (`--cfg skene_server`), rendering pages
  /// rather than running in a browser.
  pub const IS_SERVER: bool = cfg!(skene_server);

  /// Whether the build's platform is one of Apple's, iOS or macOS, whose shared
  /// vocabulary is [`apple`].
  pub const IS_APPLE: bool = matches!(Self::CURRENT, Self::Ios | Self::Macos);

  /// The platform's name, as its build names it in `skene_platform`.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Web => "web",
      Self::Ios => "ios",
      Self::Android => "android",
      Self::Macos => "macos",
      Self::Windows => "windows",
      Self::Linux => "linux",
    }
  }
}
