use skene_foundation::seam::record::{Group, Record};

use crate::{
  Button, Checkbox, Picker, PickerItem, Radio, RadioGroup, Slider, TextEditor, TextField, Toggle,
};

/// Asserts that no layout or placement property of `record` changes with a state or a signal.
fn holds_layout_still(name: &str, record: &Record) {
  for (prop, value) in record.entries() {
    if matches!(prop.group(), Group::Layout | Group::Placement) {
      assert!(!value.is_dynamic(), "{name}'s {prop:?} varies");
    }
  }
}

#[test]
fn the_built_ins_defaults_vary_paint_alone() {
  let roots = [
    ("a button", Button::new("Save").control.members.record),
    ("a switch", Toggle::new("Sync").control.members.record),
    ("a checkbox", Checkbox::new("Wi-Fi").control.members.record),
    (
      "a radio group",
      RadioGroup::new("Size").control.members.record,
    ),
    ("a radio", Radio::new("Small").members.record),
    ("a slider", Slider::new("Volume").control.members.record),
    ("a picker", Picker::new("Fruit").members.record),
    ("a picker's option", PickerItem::new("Apple").members.record),
    (
      "a text field",
      TextField::new("Name").control.members.record,
    ),
    (
      "a text editor",
      TextEditor::new("Notes").control.members.record,
    ),
  ];
  let parts = [
    ("a switch's track", Toggle::default_track()),
    ("a switch's thumb", Toggle::default_thumb()),
    ("a checkbox's face", Checkbox::default_face()),
    ("a radio's face", Radio::default_face()),
    ("a slider's track", Slider::default_track()),
    ("a slider's fill", Slider::default_fill()),
    ("a slider's thumb", Slider::default_thumb()),
    ("a picker's list", Picker::default_list()),
  ];

  for (name, record) in &roots {
    holds_layout_still(name, record);
  }

  for (name, part) in &parts {
    holds_layout_still(name, part.record());
  }
}
