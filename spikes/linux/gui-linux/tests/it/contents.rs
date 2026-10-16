//! Contents as the seam writes and reads them.

use std::rc::Rc;

use skene_foundation::seam::content::{Content, OverlayPlacement, PropValue, Side};
use skene_gui_linux::node::Contents;

#[test]
fn each_content_reads_back_what_was_written() {
  let mut contents = Contents::default();
  let samples = [
    (Content::Text, PropValue::Text("Skene".into())),
    (
      Content::Reserve,
      PropValue::Texts(Rc::from(vec![Rc::from("a"), Rc::from("bb")])),
    ),
    (Content::Placeholder, PropValue::Text("Name".into())),
    (Content::ReadOnly, PropValue::Bool(true)),
    (Content::Secure, PropValue::Bool(true)),
    (Content::Keyboard, PropValue::Text("email".into())),
    (
      Content::Placement,
      PropValue::Placement(OverlayPlacement {
        anchor: None,
        side: Side::Bottom,
        widen: true,
      }),
    ),
  ];

  for (content, value) in samples {
    contents.write(content, &value);
    assert_eq!(contents.read(content), Some(value), "{content:?}");
  }
}
