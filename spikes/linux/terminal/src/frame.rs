//! A grid of cells, as the terminal paints them, and its styled snapshot.

use std::fmt::Write as _;

use skene_foundation::value::TextDecorationStyle;

/// How one cell is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct CellStyle {
  /// Inside the selection.
  pub selected: bool,
  /// Underlined, in this style (a composition's clauses).
  pub underline: Option<TextDecorationStyle>,
  /// Showing the placeholder.
  pub placeholder: bool,
}

/// One cell: the cluster it starts (none for the second cell of a wide one, a blank for an
/// empty cell) and its style.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Cell {
  /// The cluster drawn from this cell, empty where a wide cluster before it covers it.
  pub text: String,
  /// How it is drawn.
  pub style: CellStyle,
}

impl Cell {
  /// An empty cell.
  #[must_use]
  pub fn blank() -> Self {
    Self {
      text: " ".to_owned(),
      style: CellStyle::default(),
    }
  }
}

/// A grid of cells and where the terminal's cursor stands.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Frame {
  columns: u16,
  rows: u16,
  cells: Vec<Cell>,
  /// The cursor's column and row, where it shows.
  pub cursor: Option<(u16, u16)>,
}

impl Frame {
  /// A frame of blank cells.
  #[must_use]
  pub fn new(columns: u16, rows: u16) -> Self {
    Self {
      columns,
      rows,
      cells: vec![Cell::blank(); usize::from(columns) * usize::from(rows)],
      cursor: None,
    }
  }

  /// The columns.
  #[must_use]
  pub const fn columns(&self) -> u16 {
    self.columns
  }

  /// The rows.
  #[must_use]
  pub const fn rows(&self) -> u16 {
    self.rows
  }

  /// The cell at `column`, `row`.
  #[must_use]
  pub fn cell(&self, column: u16, row: u16) -> &Cell {
    &self.cells[usize::from(row) * usize::from(self.columns) + usize::from(column)]
  }

  /// The cell at `column`, `row`, to change; `None` outside the frame.
  pub fn cell_mut(&mut self, column: u16, row: u16) -> Option<&mut Cell> {
    (column < self.columns && row < self.rows)
      .then(|| &mut self.cells[usize::from(row) * usize::from(self.columns) + usize::from(column)])
  }

  /// The frame's text: each row's clusters, its trailing blanks trimmed, each row ending in a
  /// newline.
  #[must_use]
  pub fn text(&self) -> String {
    let mut text = String::new();

    for row in 0..self.rows {
      let line: String = (0..self.columns)
        .map(|column| self.cell(column, row).text.as_str())
        .collect();

      text.push_str(line.trim_end());
      text.push('\n');
    }

    text
  }

  /// The frame as a snapshot a test compares: each row's text between bars, then its cells'
  /// styles, one character a cell (`.` plain, `s` selected, `p` placeholder, `~` dotted, `_`
  /// solid and `=` double underline, `S` selected and underlined, `>` covered by the wide
  /// cluster before), then the cursor.
  #[must_use]
  pub fn to_styled(&self) -> String {
    let mut out = String::new();

    for row in 0..self.rows {
      let text: String = (0..self.columns)
        .map(|column| {
          let cell = self.cell(column, row);

          if cell.text.is_empty() {
            ""
          } else {
            cell.text.as_str()
          }
        })
        .collect();
      let styles: String = (0..self.columns)
        .map(|column| {
          let cell = self.cell(column, row);

          match (cell.text.is_empty(), cell.style) {
            (true, _) => '>',
            (
              false,
              CellStyle {
                placeholder: true, ..
              },
            ) => 'p',
            (
              false,
              CellStyle {
                selected: true,
                underline: Some(_),
                ..
              },
            ) => 'S',
            (false, CellStyle { selected: true, .. }) => 's',
            (
              false,
              CellStyle {
                underline: Some(TextDecorationStyle::Double),
                ..
              },
            ) => '=',
            (
              false,
              CellStyle {
                underline: Some(TextDecorationStyle::Dotted),
                ..
              },
            ) => '~',
            (
              false,
              CellStyle {
                underline: Some(_), ..
              },
            ) => '_',
            (false, _) => '.',
          }
        })
        .collect();
      let _ = writeln!(out, "|{text}|");
      let _ = writeln!(out, "|{styles}|");
    }

    match self.cursor {
      Some((column, row)) => {
        let _ = writeln!(out, "cursor {column},{row}");
      }
      None => out.push_str("cursor hidden\n"),
    }

    out
  }
}
