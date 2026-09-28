use skene_reactive::{Owner, Read, Signal};

#[test]
fn read_ref_derefs_to_inner_value() {
  let signal = Signal::new(vec![10, 20, 30]);

  let read = signal.read();
  assert_eq!(read.len(), 3);
  assert_eq!(read[0], 10);
  assert_eq!(read[2], 30);
}

#[test]
fn read_untracked_also_derefs() {
  let signal = Signal::new(String::from("hello"));

  let read = signal.read_untracked();
  assert_eq!(read.as_str(), "hello");
}

#[test]
fn a_guard_outlives_its_disposed_signal() {
  let owner = Owner::new();
  let signal = owner.enter(|| Signal::new(String::from("held across disposal")));
  let read = signal.read();

  owner.dispose();
  assert_eq!(read.as_str(), "held across disposal");
  drop(read);
}
