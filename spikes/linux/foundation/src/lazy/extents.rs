//! The extents of a list's lines along its axis, with their prefix sums: where each line
//! starts and which line holds an offset, each in O(log n).

/// The extents of `n` lines along a list's axis and the spacing between neighbors, kept in a
/// Fenwick tree: a line's start, the line at an offset and a change of one line's extent
/// each cost O(log n), and building them O(n).
#[derive(Clone, Debug, Default)]
pub struct Extents {
  sizes: Vec<f64>,
  tree: Vec<f64>,
  spacing: f64,
}

impl Extents {
  /// The lines `sizes`, `spacing` apart.
  #[must_use]
  pub fn new(sizes: Vec<f64>, spacing: f64) -> Self {
    let mut tree: Vec<f64> = sizes.iter().map(|size| size + spacing).collect();

    for index in 0..tree.len() {
      let parent = index | (index + 1);

      if parent < tree.len() {
        tree[parent] += tree[index];
      }
    }

    Self {
      sizes,
      tree,
      spacing,
    }
  }

  /// How many lines.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.sizes.len()
  }

  /// Whether there are none.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.sizes.is_empty()
  }

  /// The spacing between neighbors.
  #[inline(always)]
  #[must_use]
  pub const fn spacing(&self) -> f64 {
    self.spacing
  }

  /// Line `line`'s extent; zero past the last.
  #[must_use]
  pub fn size(&self, line: usize) -> f64 {
    self.sizes.get(line).copied().unwrap_or_default()
  }

  /// Sets line `line`'s extent; nothing past the last.
  pub fn set(&mut self, line: usize, size: f64) {
    let Some(old) = self.sizes.get_mut(line) else {
      return;
    };
    let delta = size - *old;

    if delta == 0. {
      return;
    }

    *old = size;

    let mut index = line;

    while index < self.tree.len() {
      self.tree[index] += delta;
      index |= index + 1;
    }
  }

  /// Where line `line` starts, from the first line's start: the extents of the lines before
  /// it, each with the spacing after it. The whole run's end plus a spacing at `len()`.
  #[must_use]
  pub fn start(&self, line: usize) -> f64 {
    let mut sum = 0.;
    let mut end = line.min(self.tree.len());

    while end > 0 {
      sum += self.tree[end - 1];
      end &= end - 1;
    }

    sum
  }

  /// Where line `line` ends: its start and its extent.
  #[must_use]
  pub fn end(&self, line: usize) -> f64 {
    self.start(line) + self.size(line)
  }

  /// The extent of every line together, the spacing between them included.
  #[must_use]
  pub fn total(&self) -> f64 {
    if self.sizes.is_empty() {
      0.
    } else {
      self.start(self.sizes.len()) - self.spacing
    }
  }

  /// The line holding `offset` (from the first line's start), a line's spacing after it
  /// counted as its own: the first before the start, the last past the end; none where there
  /// are no lines.
  #[must_use]
  pub fn line_at(&self, offset: f64) -> Option<usize> {
    if self.sizes.is_empty() {
      return None;
    }

    // The most lines whose extents (with their spacing) sum to `offset` or less.
    let mut at = 0;
    let mut left = offset;
    let mut step = self.tree.len().next_power_of_two();

    while step > 0 {
      let next = at + step;

      if next <= self.tree.len() && self.tree[next - 1] <= left {
        at = next;
        left -= self.tree[next - 1];
      }

      step >>= 1;
    }

    Some(at.min(self.sizes.len() - 1))
  }
}

#[cfg(test)]
mod tests {
  use super::Extents;

  /// A deterministic stream of numbers in `0..1`.
  fn numbers(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed.max(1);

    move || {
      state ^= state << 13;
      state ^= state >> 7;
      state ^= state << 17;

      (state % 10_000) as f64 / 10_000.
    }
  }

  /// The straightforward sums the tree must agree with.
  fn naive_start(sizes: &[f64], spacing: f64, line: usize) -> f64 {
    sizes[..line].iter().map(|size| size + spacing).sum()
  }

  #[test]
  fn starts_and_lines_agree_with_the_sums_through_changes() {
    let mut next = numbers(7);

    for count in [0, 1, 2, 3, 7, 64, 100, 1000] {
      let spacing = (next() * 8.).floor();
      let mut sizes: Vec<f64> = (0..count).map(|_| 10. + (next() * 60.).floor()).collect();
      let mut extents = Extents::new(sizes.clone(), spacing);

      for round in 0..200 {
        if count > 0 && round % 3 == 0 {
          let line = (next() * count as f64) as usize % count;
          let size = (next() * 90.).floor();

          sizes[line] = size;
          extents.set(line, size);
        }

        let line = if count == 0 {
          0
        } else {
          (next() * count as f64) as usize % count
        };

        assert_eq!(
          extents.start(line),
          naive_start(&sizes, spacing, line.min(count))
        );

        let total: f64 = sizes.iter().sum::<f64>() + spacing * (count.max(1) - 1) as f64;

        assert_eq!(extents.total(), if count == 0 { 0. } else { total });

        let offset = next() * (total + 40.) - 20.;
        let found = extents.line_at(offset);

        match found {
          None => assert_eq!(count, 0),
          Some(found) => {
            let start = naive_start(&sizes, spacing, found);
            let after = start + sizes[found] + spacing;

            assert!(
              (offset < 0. && found == 0)
                || (start <= offset && (offset < after || found == count - 1)),
              "{offset} lies in line {found} from {start} to {after}"
            );
          }
        }
      }
    }
  }

  #[test]
  fn a_line_of_no_extent_holds_no_offset_but_its_spacing() {
    let extents = Extents::new(vec![10., 0., 10.], 0.);

    assert_eq!(extents.line_at(9.9), Some(0));
    assert_eq!(extents.line_at(10.), Some(2));
    assert_eq!(extents.start(2), 10.);
  }
}
