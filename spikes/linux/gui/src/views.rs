//! The shared views: a decorated box, the layouts, a scroll box, text, an image and a theme.
//!
//! Each property has one setter and its `derive_` twin, and conditions ride the value
//! (`set_padding(24.when(Media::MinWidth(900), 48))`). Properties come first, children last.

pub use skene_foundation::view::ScrollAxes;
use skene_foundation::{
  view::{
    self as base, ColumnBase, ContainerBase, DecorationBase, GridBase, ImageBase, MotionBase,
    RowBase, RunBase, RunWithEntries, ScrollBase, StackBase, TextBase, ThemeBase, Wraps,
  },
  wraps,
};

use crate::{Active, Axis};

/// The decorated box around at most one child: the one view that takes decoration, sizing
/// and a click.
///
/// To decorate a layout, wrap it: `Container::new().set_background(..).set_child(Column::new())`.
/// A box that handles a click is an activatable row: it takes focus, and `Enter` and `Space`
/// activate it.
///
/// ```
/// use skene_gui::prelude::*;
///
/// Container::new()
///   .set_padding(12)
///   .set_corner_radius(8)
///   .set_background(SystemColors::CONTROL.when(State::Hovered, SystemColors::CONTROL_HOVER))
///   .handle_click(|| {})
///   .set_child(Text::new("Open"));
/// ```
#[must_use]
pub struct Container(base::Container<Active>);

wraps!([] Container => base::Container<Active>, Active);

impl ContainerBase<Active> for Container {}

impl DecorationBase for Container {}

impl RunBase for Container {}

impl RunWithEntries for Container {}

impl MotionBase for Container {}

/// Children side by side, along the reading direction.
///
/// ```
/// use skene_gui::prelude::*;
///
/// Row::new()
///   .set_spacing(8)
///   .add_child(Text::new("Sync"))
///   .add_child(Toggle::new("On"));
/// ```
#[must_use]
pub struct Row(base::Row<Active>);

wraps!([] Row => base::Row<Active>, Active);

impl RowBase<Active> for Row {}

/// Children one below another.
#[must_use]
pub struct Column(base::Column<Active>);

wraps!([] Column => base::Column<Active>, Active);

impl ColumnBase<Active> for Column {}

/// Children in the cells of a grid of columns and rows, each placed by a `GridCell` or by the
/// grid's flow.
#[must_use]
pub struct Grid(base::Grid<Active>);

wraps!([] Grid => base::Grid<Active>, Active);

impl GridBase<Active> for Grid {}

/// Children over one another, the later above, each placed by a `Pinned` or by the stack's
/// item alignment.
#[must_use]
pub struct Stack(base::Stack<Active>);

wraps!([] Stack => base::Stack<Active>, Active);

impl StackBase<Active> for Stack {}

/// A box whose one child scrolls, with the platform's scrolling: its wheel and touchpad
/// motion, its scroll bars.
///
/// It would be as large as its child and can shrink to nothing, so a scroll in a column takes
/// a height, a greatest height or a `Flexible` weight to stay within the window.
///
/// A bar along each axis it scrolls shows while the content overflows it, over the content and
/// taking no room: a track ([`Scroll::default_track`]) along the box's trailing or bottom edge,
/// holding a thumb ([`Scroll::default_thumb`]) as long as the share of the content in view and
/// placed where the scroll is. The scroll box is the bar's control: a part's `Hovered` holds
/// while the pointer is over the box.
///
/// ```
/// use skene_gui::prelude::*;
///
/// // A square thumb, the default track around it.
/// let rows = Scroll::new(ScrollAxes::Vertical)
///   .set_height(200)
///   .set_thumb_builder(|axis| Scroll::default_thumb(axis).set_corner_radius(0))
///   .set_child(Column::new());
/// ```
#[must_use]
pub struct Scroll(base::Scroll<Active>);

wraps!([] Scroll => base::Scroll<Active>, Active);

impl ScrollBase<Active> for Scroll {}

impl Scroll {
  /// The default track along `axis`: 10 pixels thick, padded 3 at its sides and 2 at its
  /// ends, so its thumb runs 3 pixels in from the box's edge.
  pub fn default_track(axis: Axis) -> Container {
    Container::wrap(base::Scroll::<Active>::default_track(axis))
  }

  /// The default thumb along `axis`: 4 pixels thick and at least 20 long, its corners rounded
  /// by 2, in the ink at an alpha of 70 of 255, 110 while the pointer is over the scroll box.
  pub fn default_thumb(axis: Axis) -> Container {
    Container::wrap(base::Scroll::<Active>::default_thumb(axis))
  }
}

/// A paragraph of text. Its blanks and newlines are kept, and it wraps to its width.
#[must_use]
pub struct Text(base::Text<Active>);

wraps!([] Text => base::Text<Active>, Active);

impl TextBase<Active> for Text {}

impl RunBase for Text {}

impl MotionBase for Text {}

/// A picture.
#[must_use]
pub struct Image(base::Image<Active>);

wraps!([] Image => base::Image<Active>, Active);

impl ImageBase<Active> for Image {}

/// Defines a palette's tokens for its child and everything beneath it.
///
/// A token resolves to its definition in the nearest theme above; nesting a theme redefines
/// beneath it. A palette that changes (`Theme::derive`) is rewritten where it is defined, and
/// nothing beneath it is rebuilt.
#[must_use]
pub struct Theme(base::Theme<Active>);

wraps!([] Theme => base::Theme<Active>, Active);

impl ThemeBase<Active> for Theme {}
