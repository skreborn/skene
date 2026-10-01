use crate::spine::{Leaf, StaticLeaf, Value, values::is_identifier};

/// One grid track's size, in a template or an auto-track list.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum GridTrack<T> {
  /// A fixed length.
  Length(T),
  /// A fraction of the container's corresponding extent (`0.` to `1.`).
  Fraction(f32),
  /// A share of the free space, weighted against the other flexible tracks.
  Flex(f32),
  /// Sized to the content.
  Auto,
  /// The content's minimum size.
  MinContent,
  /// The content's maximum size.
  MaxContent,
  /// The content's maximum size, clamped to the given length.
  FitContent(T),
  /// Sized between the two limits.
  Between(TrackLimit<T>, TrackLimit<T>),
  /// As many tracks as fit, each sized between the two limits, at least one of which is
  /// a length or a fraction so the count follows from the container's size.
  ///
  /// An explicit track list holds at most one; a list holding more, or an implicit list
  /// holding any, is dropped whole, as CSS drops it.
  Adaptive(TrackLimit<T>, TrackLimit<T>),
}

impl<T: Copy> GridTrack<T> {
  /// This track repeated `count` times, for building templates.
  #[inline(always)]
  pub fn repeated(self, count: u16) -> impl Iterator<Item = Self> {
    std::iter::repeat_n(self, count as usize)
  }
}

/// A grid's explicit tracks on one axis, with names for the lines between them.
///
/// A line takes every name added where it stands: a name added before the first track
/// names the grid's start line, one added after the last track its end line. A plain
/// list of tracks is a template naming no line.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct GridTemplate<T> {
  entries: Vec<TemplateEntry<T>>,
}

/// One entry of a [`GridTemplate`], in order: a track, or a name for the line where it
/// stands.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum TemplateEntry<T> {
  /// A track.
  Track(GridTrack<T>),
  /// A name for the line between the tracks around it.
  Line(GridName),
}

impl<T> Default for GridTemplate<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T> GridTemplate<T> {
  /// A template with no track.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      entries: Vec::new(),
    }
  }

  /// This template with `track` after its entries.
  #[inline(always)]
  pub fn add_track(mut self, track: GridTrack<T>) -> Self {
    self.entries.push(TemplateEntry::Track(track));

    self
  }

  /// This template naming the line after its entries `name` too.
  #[inline(always)]
  pub fn add_line(mut self, name: GridName) -> Self {
    self.entries.push(TemplateEntry::Line(name));

    self
  }

  /// The entries, in order.
  #[inline(always)]
  pub fn entries(&self) -> &[TemplateEntry<T>] {
    &self.entries
  }

  /// The names of each line, the grid's start line first: one list per track, and one
  /// for the end line.
  #[must_use]
  pub fn line_names(&self) -> Vec<Vec<GridName>> {
    let mut lines = vec![Vec::new()];

    for entry in &self.entries {
      match entry {
        TemplateEntry::Track(_) => lines.push(Vec::new()),
        TemplateEntry::Line(name) => {
          if let Some(line) = lines.last_mut() {
            line.push(*name);
          }
        }
      }
    }

    lines
  }
}

impl<T: Copy> GridTemplate<T> {
  /// The tracks, in order.
  #[inline(always)]
  pub fn tracks(&self) -> impl Iterator<Item = GridTrack<T>> + '_ {
    self.entries.iter().filter_map(|entry| match entry {
      TemplateEntry::Track(track) => Some(*track),
      TemplateEntry::Line(_) => None,
    })
  }
}

impl<T> From<Vec<GridTrack<T>>> for GridTemplate<T> {
  /// The tracks, naming no line.
  #[inline(always)]
  fn from(tracks: Vec<GridTrack<T>>) -> Self {
    Self {
      entries: tracks.into_iter().map(TemplateEntry::Track).collect(),
    }
  }
}

impl<T> From<Vec<GridTrack<T>>> for StaticLeaf<GridTemplate<T>> {
  /// The tracks, naming no line.
  #[inline(always)]
  fn from(tracks: Vec<GridTrack<T>>) -> Self {
    Self::Fixed(GridTemplate::from(tracks))
  }
}

impl<T> From<Vec<GridTrack<T>>> for Leaf<GridTemplate<T>> {
  /// The tracks, naming no line.
  #[inline(always)]
  fn from(tracks: Vec<GridTrack<T>>) -> Self {
    Self::Static(StaticLeaf::from(tracks))
  }
}

impl<T, C> From<Vec<GridTrack<T>>> for Value<GridTemplate<T>, C> {
  /// The tracks, naming no line.
  #[inline(always)]
  fn from(tracks: Vec<GridTrack<T>>) -> Self {
    Self::new(Leaf::from(tracks))
  }
}

/// One limit of a [`GridTrack::Between`] track.
///
/// A flexible share is meaningful only as the maximum; as a minimum it behaves as
/// `Auto`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum TrackLimit<T> {
  /// A fixed length.
  Length(T),
  /// A fraction of the container's corresponding extent (`0.` to `1.`).
  Fraction(f32),
  /// A share of the free space, weighted against the other flexible tracks.
  Flex(f32),
  /// Sized to the content.
  Auto,
  /// The content's minimum size.
  MinContent,
  /// The content's maximum size.
  MaxContent,
}

/// The direction auto-placed grid items fill, and whether gaps are backfilled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum GridFlow {
  /// Fills row by row.
  Rows,
  /// Fills column by column.
  Columns,
  /// Fills row by row, backfilling earlier gaps.
  DenseRows,
  /// Fills column by column, backfilling earlier gaps.
  DenseColumns,
}

/// The name of a grid area, or of a grid line.
///
/// It is limited to ASCII letters, digits, `_` and `-`, does not start with a digit, and is
/// none of `auto`, `span`, `default` and CSS's global keywords in any case, so it is an
/// identifier wherever a platform spells it. An area named `header` names its edges
/// `header-start` and `header-end` on both axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub struct GridName(&'static str);

impl GridName {
  /// The words a name may not be, in any case.
  const RESERVED: [&'static str; 8] = [
    "auto",
    "span",
    "default",
    "initial",
    "inherit",
    "unset",
    "revert",
    "revert-layer",
  ];

  /// Names a grid area or line; debug builds check the name.
  #[inline(always)]
  pub const fn new(name: &'static str) -> Self {
    debug_assert!(
      Self::is_valid(name),
      "a grid name is ASCII letters, digits, `_` and `-`, does not start with a digit, and is \
       no reserved word",
    );

    Self(name)
  }

  /// The name.
  #[inline(always)]
  #[must_use]
  pub const fn as_str(self) -> &'static str {
    self.0
  }

  const fn is_valid(name: &str) -> bool {
    if !is_identifier(name) {
      return false;
    }

    let mut index = 0;

    while index < Self::RESERVED.len() {
      if name.eq_ignore_ascii_case(Self::RESERVED[index]) {
        return false;
      }

      index += 1;
    }

    true
  }
}

/// One end of a grid item's placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum GridPlacement {
  /// Placed by the auto-placement algorithm.
  Auto,
  /// The grid line with this index (1-based; negative counts from the end).
  Line(i16),
  /// Spans this many tracks from the other end.
  Span(u16),
  /// The `n`th line named `name` (1-based; negative counts from the end); an area's name
  /// finds the area's start edge at the start and its end edge at the end.
  Named(GridName, i16),
  /// Spans to the `n`th line named `name` from the other end.
  NamedSpan(GridName, u16),
}

/// A start and an end on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct Line<T> {
  /// The start.
  pub start: T,
  /// The end.
  pub end: T,
}

impl Line<GridPlacement> {
  /// A placement starting at `line`, ending by auto-placement.
  #[inline(always)]
  pub const fn at(line: i16) -> Self {
    Self {
      start: GridPlacement::Line(line),
      end: GridPlacement::Auto,
    }
  }

  /// A placement spanning `count` tracks from wherever auto-placement starts it.
  #[inline(always)]
  pub const fn span(count: u16) -> Self {
    Self {
      start: GridPlacement::Auto,
      end: GridPlacement::Span(count),
    }
  }

  /// A placement from `start` to `end`, as 1-based grid lines.
  #[inline(always)]
  pub const fn between(start: i16, end: i16) -> Self {
    Self {
      start: GridPlacement::Line(start),
      end: GridPlacement::Line(end),
    }
  }

  /// A placement over the area named `name`, from its start edge to its end edge.
  #[inline(always)]
  pub const fn area(name: GridName) -> Self {
    Self {
      start: GridPlacement::Named(name, 1),
      end: GridPlacement::Named(name, 1),
    }
  }
}

impl From<GridName> for Line<GridPlacement> {
  /// A placement over the area named `name`.
  #[inline(always)]
  fn from(name: GridName) -> Self {
    Self::area(name)
  }
}

impl From<GridName> for StaticLeaf<Line<GridPlacement>> {
  /// A placement over the area named `name`.
  #[inline(always)]
  fn from(name: GridName) -> Self {
    Self::Fixed(Line::area(name))
  }
}

impl From<GridName> for Leaf<Line<GridPlacement>> {
  /// A placement over the area named `name`.
  #[inline(always)]
  fn from(name: GridName) -> Self {
    Self::Static(StaticLeaf::from(name))
  }
}

impl<C> From<GridName> for Value<Line<GridPlacement>, C> {
  /// A placement over the area named `name`.
  #[inline(always)]
  fn from(name: GridName) -> Self {
    Self::new(Leaf::from(name))
  }
}

/// Named areas over a grid's cells, row by row.
///
/// Each row names its cells in order, separated by spaces, a `.` marking a cell no area
/// covers. Every row has as many cells and each name covers a filled rectangle; a
/// template breaking either rule is dropped whole, as CSS drops it, and a debug build
/// panics on it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub struct GridAreas {
  rows: Vec<Vec<Option<GridName>>>,
}

impl GridAreas {
  /// The areas `rows` spell, top row first: `GridAreas::new(["header header", "nav main"])`.
  pub fn new(rows: impl IntoIterator<Item = &'static str>) -> Self {
    let rows = rows
      .into_iter()
      .map(|row| {
        row
          .split_ascii_whitespace()
          .map(|cell| (!cell.bytes().all(|byte| byte == b'.')).then(|| GridName::new(cell)))
          .collect()
      })
      .collect();
    let areas = Self { rows };

    debug_assert!(
      areas.areas().is_some(),
      "grid areas need rows of one length and a filled rectangle per name"
    );

    areas
  }

  /// The cells, row by row; `None` for a cell no area covers.
  #[inline(always)]
  #[must_use]
  pub fn rows(&self) -> &[Vec<Option<GridName>>] {
    &self.rows
  }

  /// Each area in the order its name first appears, bounded by 1-based grid lines;
  /// `None` when the template breaks a rule.
  #[must_use]
  pub fn areas(&self) -> Option<Vec<GridArea>> {
    let width = self.rows.first().map_or(0, Vec::len);

    if self.rows.iter().any(|row| row.len() != width) {
      return None;
    }

    let mut areas: Vec<GridArea> = Vec::new();

    for (row, cells) in self.rows.iter().enumerate() {
      for (column, cell) in cells.iter().enumerate() {
        let Some(name) = *cell
        else {
          continue;
        };
        let (row, column) = (u16::try_from(row).ok()?, u16::try_from(column).ok()?);

        match areas.iter_mut().find(|area| area.name == name) {
          Some(area) => {
            area.rows.start = area.rows.start.min(row + 1);
            area.rows.end = area.rows.end.max(row + 2);
            area.columns.start = area.columns.start.min(column + 1);
            area.columns.end = area.columns.end.max(column + 2);
          }
          None => areas.push(GridArea {
            name,
            rows: Line {
              start: row + 1,
              end: row + 2,
            },
            columns: Line {
              start: column + 1,
              end: column + 2,
            },
          }),
        }
      }
    }

    let filled = areas.iter().all(|area| {
      (area.rows.start..area.rows.end).all(|row| {
        (area.columns.start..area.columns.end)
          .all(|column| self.rows[usize::from(row - 1)][usize::from(column - 1)] == Some(area.name))
      })
    });

    filled.then_some(areas)
  }
}

/// One named area of a [`GridAreas`], bounded by 1-based grid lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct GridArea {
  /// The area's name.
  pub name: GridName,
  /// The row lines bounding it.
  pub rows: Line<u16>,
  /// The column lines bounding it.
  pub columns: Line<u16>,
}

#[cfg(test)]
mod tests {
  use super::{GridArea, GridAreas, GridName, GridTemplate, GridTrack, Line};

  /// Each area is bounded by the lines around the cells its name covers, in the order
  /// the names first appear.
  #[test]
  fn areas_bound_their_cells() {
    let (head, nav, main) = (
      GridName::new("head"),
      GridName::new("nav"),
      GridName::new("main"),
    );
    let area = |name, rows: (u16, u16), columns: (u16, u16)| GridArea {
      name,
      rows: Line {
        start: rows.0,
        end: rows.1,
      },
      columns: Line {
        start: columns.0,
        end: columns.1,
      },
    };

    assert_eq!(
      GridAreas::new(["head head", "nav main", "nav ."]).areas(),
      Some(vec![
        area(head, (1, 2), (1, 3)),
        area(nav, (2, 4), (1, 2)),
        area(main, (2, 3), (2, 3)),
      ])
    );
  }

  /// A template with rows of different lengths, or a name covering no filled rectangle,
  /// has no areas.
  #[test]
  fn a_broken_template_has_no_areas() {
    let (a, b) = (Some(GridName::new("a")), Some(GridName::new("b")));
    let ragged = GridAreas {
      rows: vec![vec![a, a], vec![a]],
    };
    let bent = GridAreas {
      rows: vec![vec![a, a], vec![a, b]],
    };
    let split = GridAreas {
      rows: vec![vec![a, b, a]],
    };

    assert_eq!(ragged.areas(), None);
    assert_eq!(bent.areas(), None);
    assert_eq!(split.areas(), None);
  }

  /// A template names each line with the names added where it stands, the start line
  /// before the first track and the end line after the last.
  #[test]
  fn a_template_names_the_lines_its_names_stand_at() {
    let (full, content, wide) = (
      GridName::new("full"),
      GridName::new("content"),
      GridName::new("wide"),
    );
    let template = GridTemplate::new()
      .add_line(full)
      .add_track(GridTrack::<f32>::Flex(1.))
      .add_line(content)
      .add_line(wide)
      .add_track(GridTrack::Length(600.))
      .add_track(GridTrack::Flex(1.))
      .add_line(full);

    assert_eq!(
      template.line_names(),
      [vec![full], vec![content, wide], vec![], vec![full]]
    );
    assert_eq!(template.tracks().count(), 3);
    assert_eq!(
      GridTemplate::from(vec![GridTrack::<f32>::Flex(1.); 2]).line_names(),
      [vec![], vec![], vec![]]
    );
  }

  /// A name is an identifier and no reserved word, in any case.
  #[test]
  fn a_name_is_an_identifier_and_no_reserved_word() {
    assert!(GridName::is_valid("main-2_x"));
    assert!(!GridName::is_valid("2col"));
    assert!(!GridName::is_valid("span"));
    assert!(!GridName::is_valid("Auto"));
    assert!(!GridName::is_valid("revert-layer"));
    assert!(!GridName::is_valid("a.b"));
  }
}
