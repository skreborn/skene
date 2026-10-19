use skene_foundation::{
  seam::record::{Group, Record},
  view::{HasRecord, Wraps},
};

use crate::{
  Button, Checkbox, Picker, PickerItem, Radio, RadioGroup, Slider, TextEditor, TextField, Toggle,
  prelude::*,
};

/// The record `view`'s setters wrote: its control's root's, for a styled control.
fn record<V: Wraps<Inner: HasRecord>>(view: V) -> Record {
  view.into_inner().record_mut().clone()
}

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
  let records = [
    ("a button", record(Button::new("Save"))),
    ("a switch", record(Toggle::new("Sync"))),
    ("a checkbox", record(Checkbox::new("Wi-Fi"))),
    ("a radio group", record(RadioGroup::<u8>::new("Size"))),
    ("a radio", record(Radio::new("Small"))),
    ("a slider", record(Slider::new("Volume"))),
    ("a picker", record(Picker::<u8>::new("Fruit"))),
    ("a picker's option", record(PickerItem::new("Apple"))),
    ("a text field", record(TextField::new("Name"))),
    ("a text editor", record(TextEditor::new("Notes"))),
    ("a switch's track", record(Toggle::default_track())),
    ("a switch's thumb", record(Toggle::default_thumb())),
    ("a checkbox's face", record(Checkbox::default_face())),
    ("a radio's face", record(Radio::default_face())),
    ("a slider's track", record(Slider::default_track())),
    ("a slider's fill", record(Slider::default_fill())),
    ("a slider's thumb", record(Slider::default_thumb())),
    ("a picker's list", record(Picker::<u8>::default_list())),
  ];

  for (name, record) in &records {
    holds_layout_still(name, record);
  }
}
