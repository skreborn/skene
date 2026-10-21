//! taffy 0.15.0: a single-line flex row of definite height measures each stretched item's height
//! (its hypothetical cross size, CSS Flexbox 9.4 step 7) though step 11 then sets every such
//! item's height to the line's, which is the container's inner height: the measurement is never
//! read. A leaf whose height costs a text layout per width pays for one it does not need.
use std::cell::RefCell;

use taffy::{
  AvailableSpace, LayoutInput, LayoutOutput, RequestedAxis, RunMode, Size, Style, TaffyTree,
  compute_leaf_layout, prelude::{TaffyMaxContent, length},
};

fn main() {
  let mut tree: TaffyTree<usize> = TaffyTree::new();
  let leaves: Vec<_> = (0..3)
    .map(|index| tree.new_leaf_with_context(Style::default(), index).expect("leaf"))
    .collect();
  let row = tree
    .new_with_children(
      Style {
        size: Size { width: length(600.), height: length(100.) },
        ..Style::default()
      },
      &leaves,
    )
    .expect("row");
  let asked = RefCell::new(Vec::new());

  tree
    .compute_layout_with_measure(row, Size::MAX_CONTENT, |inputs: LayoutInput, _, context, style| {
      let index = context.map_or(usize::MAX, |index| *index);
      asked.borrow_mut().push((index, inputs.run_mode, inputs.axis, inputs.known_dimensions, inputs.available_space));

      compute_leaf_layout(inputs, style, |_, _| 0., |known, _| Size {
        width: known.width.unwrap_or(150.),
        // As tall as a text 300 wide wrapped at the width it is given, in lines 20 tall.
        height: known.height.unwrap_or_else(|| 20. * (300. / known.width.unwrap_or(150.)).ceil()),
      })
    })
    .expect("layout");

  let asked = asked.into_inner();
  let height_asks = asked
    .iter()
    .filter(|(_, mode, axis, known, _)| {
      *mode == RunMode::ComputeSize && *axis != RequestedAxis::Horizontal && known.height.is_none()
    })
    .count();

  for (index, mode, axis, known, available) in &asked {
    let show = |space: AvailableSpace| match space {
      AvailableSpace::Definite(value) => format!("{value}"),
      AvailableSpace::MinContent => "min-content".into(),
      AvailableSpace::MaxContent => "max-content".into(),
    };
    println!(
      "item {index}: {mode:?} {axis:?} known {:?}x{:?} available {}x{}",
      known.width, known.height, show(available.width), show(available.height)
    );
  }

  println!("measure calls: {}, of which an unknown height measured: {height_asks}", asked.len());

  for leaf in &leaves {
    let layout = tree.layout(*leaf).expect("laid out");
    println!("item laid out at {}x{}", layout.size.width, layout.size.height);
  }

  let _: Option<LayoutOutput> = None;
}
