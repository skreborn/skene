//! Context: values provided by an owner, visible to its subtree, shadowed by
//! descendants.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_reactive::{Effect, Get, Owner, Set, Signal, provide_context, use_context};

#[test]
fn context_in_same_owner() {
  Owner::new().enter(|| {
    provide_context(42i32);
    assert_eq!(use_context::<i32>(), Some(42));
  });
}

#[test]
fn context_inherited_and_shadowed() {
  let parent = Owner::new();

  parent.enter(|| {
    provide_context(42i32);

    let child = parent.child();

    child.enter(|| {
      assert_eq!(use_context::<i32>(), Some(42), "inherited from the parent");

      provide_context(100i32);
      assert_eq!(use_context::<i32>(), Some(100), "shadowed for the subtree");
    });

    assert_eq!(use_context::<i32>(), Some(42), "the parent keeps its own");
  });
}

#[test]
fn sibling_owners_are_isolated() {
  let parent = Owner::new();

  parent.enter(|| {
    provide_context(0i32);

    let first = parent.child();
    let second = parent.child();

    first.enter(|| provide_context(1i32));
    second.enter(|| provide_context(2i32));

    first.enter(|| assert_eq!(use_context::<i32>(), Some(1)));
    second.enter(|| assert_eq!(use_context::<i32>(), Some(2)));
  });
}

#[test]
fn deeply_nested_lookup_walks_to_the_root() {
  let root = Owner::new();

  root.enter(|| {
    provide_context(String::from("root"));

    let mut owner = Owner::current();

    for _ in 0..3 {
      owner = owner.child();
    }

    owner.enter(|| {
      assert_eq!(use_context::<String>(), Some(String::from("root")));
    });
  });
}

#[test]
fn context_gone_after_dispose() {
  let parent = Owner::new();
  let child = parent.child();

  parent.enter(|| provide_context(42i32));
  child.enter(|| provide_context(100i32));

  child.dispose();

  parent.enter(|| {
    assert_eq!(
      use_context::<i32>(),
      Some(42),
      "the parent's context survives"
    );
  });
}

#[test]
fn context_not_visible_to_parent() {
  let parent = Owner::new();
  let child = parent.child();

  child.enter(|| provide_context(100i32));

  parent.enter(|| assert_eq!(use_context::<i32>(), None));
}

#[test]
fn newtype_wrappers_are_distinct_types() {
  #[derive(Debug, Clone, PartialEq)]
  struct UserId(i32);

  #[derive(Debug, Clone, PartialEq)]
  struct PostId(i32);

  Owner::new().enter(|| {
    provide_context(UserId(1));
    provide_context(PostId(2));

    assert_eq!(use_context::<UserId>(), Some(UserId(1)));
    assert_eq!(use_context::<PostId>(), Some(PostId(2)));
    assert_eq!(use_context::<i32>(), None);
  });
}

#[test]
fn overwrite_in_same_owner() {
  Owner::new().enter(|| {
    provide_context(42i32);
    provide_context(100i32);
    assert_eq!(use_context::<i32>(), Some(100));
  });
}

#[test]
fn context_visible_in_effect() {
  let owner = Owner::new();
  let seen = Rc::new(Cell::new(0i32));

  owner.enter(|| {
    provide_context(42i32);

    let trigger = Signal::new(0);

    {
      let seen = seen.clone();

      Effect::new(move |_| {
        trigger.get();

        if let Some(value) = use_context::<i32>() {
          seen.set(value);
        }
      });
    }

    trigger.set(1);
  });

  assert_eq!(seen.get(), 42);
}

#[test]
fn context_with_signal_handle() {
  Owner::new().enter(|| {
    let source = Signal::new(42);

    provide_context(source);

    let retrieved = use_context::<Signal<i32>>().unwrap();

    assert_eq!(retrieved.get_untracked(), 42);

    retrieved.set(100);
    assert_eq!(source.get_untracked(), 100);
  });
}

#[test]
fn a_rerun_drops_the_contexts_its_last_run_provided() {
  let provide = Signal::new(true);
  let seen = Rc::new(RefCell::new(Vec::new()));

  Owner::new().enter(|| {
    let seen = seen.clone();

    Effect::new(move |_: Option<()>| {
      if provide.get() {
        provide_context(7u32);
      }

      seen.borrow_mut().push(use_context::<u32>());
    });
  });

  provide.set(false);

  assert_eq!(&*seen.borrow(), &[Some(7), None]);
}
