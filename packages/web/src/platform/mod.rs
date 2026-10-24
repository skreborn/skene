//! Each pixel platform's own vocabulary, a module per platform.
//!
//! A platform's module holds what only that platform has: its data types exist in every
//! build, so code naming one compiles wherever the facade does, and its controls exist
//! in its own build alone, so code naming one is written inside an arm that matches its
//! platform, under `#[cfg(skene_platform = "<p>")]`. [`apple`] is the one grouping, for
//! what iOS and macOS share. The foundation names no platform: everything here belongs
//! to the facade. An app that diverges per platform matches on
//! [`Platform::CURRENT`](crate::Platform::CURRENT).

pub mod web;

/// Android's own vocabulary: its data types in every build, its controls in the
/// `android` build alone. It holds nothing yet.
pub mod android {}

/// What iOS and macOS share ([`Platform::IS_APPLE`](crate::Platform::IS_APPLE)): its data types in
/// every build, its controls in the `ios` and `macos` builds alone. It holds nothing yet.
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
