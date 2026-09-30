//! Enhancement in cells: a form swaps on the signal it waits for, and nothing on the
//! terminal facade names a render made without script.

use skene_tui::{self as tui, prelude::*, testing};

use crate::common::with_platform;

#[test]
fn an_enhancement_swaps_on_its_signal() {
  with_platform(|| {
    let ready = Signal::new(false);
    let _ = tui::mount(move || {
      Enhance::new(ready.read_only())
        .set_fallback(|| Text::new("waiting"))
        .set_child_builder(|| Text::new("ready"))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(20, 4);

    assert!(
      frame.contains("waiting") && !frame.contains("ready"),
      "{frame}"
    );

    ready.set(true);

    let frame = testing::render_to_text(20, 4);

    assert!(
      frame.contains("ready") && !frame.contains("waiting"),
      "{frame}"
    );
  });
}
