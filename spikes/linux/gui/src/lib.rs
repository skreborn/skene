//! # skene-gui
//!
//! Skene for the pixel platforms: the web, iOS, Android, macOS, Windows and Linux. An app is
//! plain code: concrete views, plain numbers in pixels, and one entry.
//!
//! ```
//! use skene_gui::prelude::*;
//!
//! fn settings() -> impl View {
//!   let sync = Signal::new(true);
//!
//!   Column::new()
//!     .set_padding(24)
//!     .set_spacing(12.when(Media::min_width(900), 20))
//!     .add_child(Text::new("Settings").set_font_size(28))
//!     .add_child(Toggle::new("Sync").bind_on(sync))
//! }
//! ```
//!
//! **One platform per build, one realization per platform.** The build names its platform with
//! `--cfg skene_platform="<platform>"` on the platform's real target (the `.cargo/config.toml`
//! that `skene new` writes does), so plain `cargo check`, `cargo test`, `cargo run`, clippy,
//! rustdoc and rust-analyzer need nothing else; a build that names none fails, saying how to
//! name one. There is no headless build and no test double: every platform has exactly one
//! realization, the real one, and [`Platform::CURRENT`] is never anything but a platform. In
//! this prototype only Linux (drawn by Skene) is realized; the other platforms fail to build,
//! saying so.
//!
//! **Platform differences are as small as they are.** Plain Rust branches on
//! [`Platform::CURRENT`]; what one platform alone has lives in [`platform`]
//! (`platform::linux`), in that platform's build alone; the controls ([`Toggle`], [`Checkbox`],
//! [`RadioGroup`], [`Button`], [`TextField`], [`TextEditor`], [`Slider`], [`Picker`]) are Skene's, drawn on every
//! platform.

#[cfg(not(any(
  skene_platform = "web",
  skene_platform = "server",
  skene_platform = "ios",
  skene_platform = "android",
  skene_platform = "macos",
  skene_platform = "windows",
  skene_platform = "linux",
)))]
compile_error!(
  "skene-gui builds for one platform, which the build names: add `--cfg skene_platform=\"linux\"` (and its `--check-cfg`) to the target's rustflags, as the `.cargo/config.toml` that `skene new` writes does"
);

#[cfg(skene_platform = "web")]
compile_error!("web is not realized in this prototype");

#[cfg(skene_platform = "server")]
compile_error!("server is not realized in this prototype");

#[cfg(skene_platform = "ios")]
compile_error!("ios is not realized in this prototype");

#[cfg(skene_platform = "android")]
compile_error!("android is not realized in this prototype");

#[cfg(skene_platform = "macos")]
compile_error!("macos is not realized in this prototype");

#[cfg(skene_platform = "windows")]
compile_error!("windows is not realized in this prototype");

#[cfg(skene_platform = "linux")]
mod control;
#[cfg(skene_platform = "linux")]
mod controls;
#[cfg(skene_platform = "linux")]
mod lazy;
#[cfg(skene_platform = "linux")]
pub mod media;
#[cfg(skene_platform = "linux")]
mod overlay;
#[cfg(skene_platform = "linux")]
pub mod painted;
#[cfg(skene_platform = "linux")]
pub mod platform;
#[cfg(skene_platform = "linux")]
pub mod prelude;
#[cfg(skene_platform = "linux")]
mod runtime;
#[cfg(skene_platform = "linux")]
pub mod testing;
#[cfg(skene_platform = "linux")]
mod view;
#[cfg(skene_platform = "linux")]
mod views;
#[cfg(skene_platform = "linux")]
mod wrappers;

#[doc(hidden)]
#[cfg(skene_platform = "linux")]
pub mod __private;

/// What a control's interaction is: the shipped behaviors, the trait a library's own
/// implements, the handle a behavior acts through and the key tables every platform shares.
pub use skene_foundation::behavior;
pub use skene_foundation::{
  Alignment, Angle, Arrangement, AxisAlignment, Border, BorderStyle, Color, Condition, Corners,
  Curve, Definitions, Dimension, Edges, Effect, Environment, Expr, Fill, Flexibility, Flow,
  FontFamily, FontStyle, FontWeight, GridPlacement, GridTrack, Inset, IntoUnit, LinearGradient,
  Media, Memo, Offset, Owner, RadialGradient, Ring, Scheme, Shadow, Shadows, Signal, Spacing,
  State, StateSet, StopColor, SweepGradient, SystemColors, TextAlignment, TextDecoration,
  TextDecorationStyle, Token, TokenRef, TokenSet, Tracks, UnitPoint, Value, When, batch,
  layout::{Axis, Rect},
  lazy::{GridCells, ItemExtent, ScrollPosition},
  seam::{
    access::{AccessFacts, Checked, RangeValue, Role, Shortcut},
    content::{Range, Side, TextSelection},
    control::{
      AccessRequest, Availability, ClipboardOp, ControlState, DragAxes, Element, LabelPlacement,
      Part, Prevent,
    },
    record::{Animation, Iterations, Key, Keyframe, Transition, keys},
    report::{
      ContextMenuEvent, Dropped, KeyEvent, MenuSource, Modifiers, PointerEvent, PointerPhase,
    },
    tree::Bubbling,
  },
  untrack,
};
/// Marks `fn app() -> impl View` as the app's root and emits the entry the build's platform
/// calls: on Linux, the binary's `main`.
pub use skene_gui_macros::main;
/// Runs a plain `#[test]` body under an owner of its own, the environment reset first.
pub use skene_gui_macros::test;

#[cfg(skene_platform = "linux")]
pub use crate::{
  control::{
    AsPart, BasicButton, BasicCheckbox, BasicListbox, BasicListboxOption, BasicPicker, BasicRadio,
    BasicRadioGroup, BasicSlider, BasicTextEditor, BasicTextField, Control,
  },
  controls::{
    Button, Checkbox, ContextMenu, Keyboard, Listbox, ListboxItem, MenuEntry, MenuItem, Picker,
    PickerItem, Radio, RadioGroup, Slider, TextEditor, TextField, Toggle, marks,
  },
  lazy::{LazyColumn, LazyHorizontalGrid, LazyRow, LazyVerticalGrid},
  overlay::Overlay,
  painted::{Canvas, Paint, Painted},
  runtime::{Resource, Sleep, schedule, sleep, spawn},
  view::{AnyView, Apply, ChildOf, Cx, Derived, Label, Node, NodeHandle, Painting, View},
  views::{Column, Container, Grid, Image, Row, Scroll, ScrollAxes, Stack, Text, Theme},
  wrappers::{Flexible, GridCell, Pinned},
};
/// What a choice's options carry and what a list box chooses: a value of the app's own type.
pub use skene_foundation::behavior::{Choice, Chosen};
/// The traits the views take their members from, each member written once in the
/// foundation: a view's own (`ContainerBase`), a surface several views share
/// (`DecorationBase`, `RunBase`, `MotionBase`, `ControlMembersBase`), and what a capability
/// adds (`RunWithEntries`). The prelude carries them, so a view's members are in scope where
/// its type is.
pub use skene_foundation::view::{
  AnyKind, AsPartBase, ButtonBase, CheckboxBase, ChoiceBase, ColumnBase, ContainerBase,
  ContextMenuBase, ControlBase, ControlMembersBase, DecorationBase, DerivedBase, EditingBase,
  FlexibleBase, GridBase, GridCellBase, ImageBase, ItemHeightBase, ItemWidthBase, LazyBase,
  ListboxBase, ListboxItemBase, MenuItemBase, MotionBase, OptionsBase, OverlayBase, PaintedBase,
  PickerBase, PickerItemBase, PinnedBase, RadioBase, RadioGroupBase, RowBase, RunBase,
  RunWithEntries, ScrollBase, SliderBase, StackBase, TextBase, TextEditorBase, TextFieldBase,
  ThemeBase, ToggleBase,
};

/// The realization this build is for: what a library's own view implements
/// `skene_foundation::view::View` for, building what it is made of as a built-in does.
///
/// ```
/// use skene_foundation::view::AnyKind;
/// use skene_gui::{Active, prelude::*};
///
/// struct Badge {
///   label: String,
/// }
///
/// impl AnyKind for Badge {}
///
/// impl skene_foundation::view::View<Active> for Badge {
///   fn build(self, cx: &mut Cx) -> Node {
///     Container::new()
///       .set_padding(4)
///       .set_child(Text::new(self.label))
///       .build(cx)
///   }
/// }
/// ```
#[cfg(skene_platform = "linux")]
pub type Active = skene_gui_linux::Linux;

/// The platform this build is for.
///
/// It is a constant: `match Platform::CURRENT { .. }` (or `matches!`) compiles only the arm
/// that holds, even unoptimized. Every arm type-checks in every build, so an arm uses what
/// every build has; a platform's own controls are named under its `cfg`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Platform {
  /// The web, in the browser and on the server.
  Web,
  /// iOS.
  Ios,
  /// Android.
  Android,
  /// macOS.
  Macos,
  /// Windows.
  Windows,
  /// Linux, drawn by Skene.
  Linux,
}

impl Platform {
  /// The platform this build is for. A build that names no platform does not compile, so
  /// this is always the platform the build names.
  pub const CURRENT: Self = if cfg!(any(skene_platform = "web", skene_platform = "server")) {
    Self::Web
  } else if cfg!(skene_platform = "ios") {
    Self::Ios
  } else if cfg!(skene_platform = "android") {
    Self::Android
  } else if cfg!(skene_platform = "macos") {
    Self::Macos
  } else if cfg!(skene_platform = "windows") {
    Self::Windows
  } else {
    Self::Linux
  };

  /// Whether this build is for one of Apple's platforms (iOS, macOS).
  pub const IS_APPLE: bool = matches!(Self::CURRENT, Self::Ios | Self::Macos);

  /// The platform's name, as the `cfg` names it.
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
