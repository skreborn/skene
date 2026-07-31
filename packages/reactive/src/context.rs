use std::any::{Any, TypeId};

use crate::runtime::RUNTIME;

/// A marker type for context operations.
///
/// Provides static methods for storing and retrieving context values
/// in the reactive scope hierarchy.
///
/// # Example
/// ```rust
/// # use skene_reactive::{Context, Scope};
/// let scope = Scope::new();
/// scope.enter(|| {
///   Context::provide(42i32);
///   Context::provide(String::from("Hello"));
///
///   assert_eq!(Context::get::<i32>(), Some(42));
///   assert_eq!(Context::get::<String>(), Some(String::from("Hello")));
/// });
/// ```
pub struct Context;

impl Context {
  /// Stores a context value in the current scope.
  ///
  /// The stored context value can be retrieved by the current scope and any of its
  /// descendants using [`Context::get`]. Child scopes can provide their own values
  /// of the same type, which will shadow the parent's value for that subtree.
  ///
  /// `Context` values are automatically cleaned up when the scope is disposed.
  ///
  /// # Example
  /// ```rust
  /// # use skene_reactive::{Context, Scope};
  /// let scope = Scope::new();
  /// scope.enter(|| {
  ///   Context::provide(42i32);
  ///   assert_eq!(Context::get::<i32>(), Some(42));
  /// });
  /// ```
  pub fn provide<T>(value: T)
  where
    T: Clone + 'static,
  {
    let ty = TypeId::of::<T>();

    RUNTIME.with(|runtime| {
      let scope = *runtime.current_scope.borrow();
      runtime
        .entities
        .borrow_mut()
        .entry(scope)
        .or_default()
        .contexts
        .get_or_insert_default()
        .insert(ty, Box::new(value) as Box<dyn Any>);
    });
  }

  /// Tries to retrieve a stored context value from the current scope or its ancestors.
  ///
  /// `Context` lookup walks up the scope tree from the current scope to find the
  /// nearest ancestor that provides a value of the requested type. This enables
  /// nested components to override context values for their subtrees.
  ///
  /// # Example
  /// ```rust
  /// # use skene_reactive::{Context, Scope};
  /// let parent = Scope::new();
  /// parent.enter(|| {
  ///   Context::provide(42i32);
  ///
  ///   let child = parent.create_child();
  ///   child.enter(|| {
  ///     // Child sees parent's context
  ///     assert_eq!(Context::get::<i32>(), Some(42));
  ///   });
  /// });
  /// ```
  pub fn get<T>() -> Option<T>
  where
    T: Clone + 'static,
  {
    let ty = TypeId::of::<T>();

    RUNTIME.with(|runtime| {
      let mut scope = *runtime.current_scope.borrow();
      let entities = runtime.entities.borrow();

      loop {
        let entity = entities.get(&scope)?;

        if let Some(contexts) = &entity.contexts
          && let Some(value) = contexts.get(&ty)
        {
          return value.downcast_ref::<T>().cloned();
        }

        match entity.parent {
          Some(parent) => scope = parent,
          None => return None,
        }
      }
    })
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use super::*;
  use crate::{Effect, RwSignal, SignalGet, SignalUpdate, id::Id, scope::Scope};

  #[test]
  fn context_in_same_scope() {
    let scope = Scope::new();
    scope.enter(|| {
      Context::provide(42i32);
      assert_eq!(Context::get::<i32>(), Some(42));
    });
  }

  #[test]
  fn context_inherited_from_parent() {
    let parent = Scope::new();
    parent.enter(|| {
      Context::provide(42i32);

      let child = parent.create_child();
      child.enter(|| {
        // Child should see parent's context
        assert_eq!(Context::get::<i32>(), Some(42));
      });
    });
  }

  #[test]
  fn context_shadowing_in_child() {
    let parent = Scope::new();
    parent.enter(|| {
      Context::provide(42i32);

      let child = parent.create_child();
      child.enter(|| {
        // Override in child scope
        Context::provide(100i32);
        assert_eq!(Context::get::<i32>(), Some(100));
      });

      // Parent still has original value
      assert_eq!(Context::get::<i32>(), Some(42));
    });
  }

  #[test]
  fn sibling_scopes_isolated() {
    let parent = Scope::new();
    parent.enter(|| {
      Context::provide(0i32);

      let child1 = parent.create_child();
      let child2 = parent.create_child();

      child1.enter(|| {
        Context::provide(1i32);
        assert_eq!(Context::get::<i32>(), Some(1));
      });

      child2.enter(|| {
        Context::provide(2i32);
        assert_eq!(Context::get::<i32>(), Some(2));
      });

      // Verify they're still isolated
      child1.enter(|| {
        assert_eq!(Context::get::<i32>(), Some(1));
      });

      child2.enter(|| {
        assert_eq!(Context::get::<i32>(), Some(2));
      });
    });
  }

  #[test]
  fn context_cleaned_up_on_dispose() {
    let parent = Scope::new();
    let child_value = parent.enter(|| {
      Context::provide(42i32);

      let child = parent.create_child();
      let value = child.enter(|| {
        Context::provide(100i32);
        Context::get::<i32>()
      });

      // Dispose child
      child.dispose();

      value
    });

    assert_eq!(child_value, Some(100));

    // After dispose, parent context should still work
    parent.enter(|| {
      assert_eq!(Context::get::<i32>(), Some(42));
    });
  }

  #[test]
  fn deeply_nested_context_lookup() {
    let root = Scope::new();
    root.enter(|| {
      Context::provide(String::from("root"));

      let level1 = root.create_child();
      level1.enter(|| {
        let level2 = level1.create_child();
        level2.enter(|| {
          let level3 = level2.create_child();
          level3.enter(|| {
            // Should find root's context 3 levels up
            assert_eq!(Context::get::<String>(), Some(String::from("root")));
          });
        });
      });
    });
  }

  #[test]
  fn multiple_context_types() {
    let scope = Scope::new();
    scope.enter(|| {
      Context::provide(42i32);
      Context::provide(String::from("hello"));
      Context::provide(3.15f64);

      assert_eq!(Context::get::<i32>(), Some(42));
      assert_eq!(Context::get::<String>(), Some(String::from("hello")));
      assert_eq!(Context::get::<f64>(), Some(3.15));
      assert_eq!(Context::get::<bool>(), None);
    });
  }

  #[test]
  fn context_not_found_returns_none() {
    let scope = Scope::new();
    scope.enter(|| {
      // No context provided, should return None
      assert_eq!(Context::get::<i32>(), None);
      assert_eq!(Context::get::<String>(), None);
    });
  }

  #[test]
  fn overwrite_context_in_same_scope() {
    let scope = Scope::new();
    scope.enter(|| {
      Context::provide(42i32);
      assert_eq!(Context::get::<i32>(), Some(42));

      // Overwrite in same scope
      Context::provide(100i32);
      assert_eq!(Context::get::<i32>(), Some(100));
    });
  }

  #[test]
  fn parent_disposal_cleans_up_children() {
    let parent = Scope::new();
    let child = parent.create_child();

    parent.enter(|| {
      Context::provide(42i32);
    });

    child.enter(|| {
      Context::provide(100i32);
    });

    // Verify contexts exist
    parent.enter(|| {
      assert_eq!(Context::get::<i32>(), Some(42));
    });
    child.enter(|| {
      assert_eq!(Context::get::<i32>(), Some(100));
    });

    // Dispose parent - should clean up child too
    parent.dispose();

    // Verify parent's context is gone (scope no longer has context)
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&parent.0)
          .and_then(|entity| entity.contexts.as_ref())
          .is_none()
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child.0)
          .and_then(|entity| entity.contexts.as_ref())
          .is_none()
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child.0)
          .and_then(|entity| entity.parent)
          .is_none()
      );
    });
  }

  #[test]
  fn context_shadowing_at_multiple_levels() {
    let root = Scope::new();
    root.enter(|| {
      Context::provide(String::from("root"));

      let level1 = root.create_child();
      level1.enter(|| {
        // Don't provide at level1, should inherit from root
        assert_eq!(Context::get::<String>(), Some(String::from("root")));

        let level2 = level1.create_child();
        level2.enter(|| {
          // Shadow at level2
          Context::provide(String::from("level2"));
          assert_eq!(Context::get::<String>(), Some(String::from("level2")));

          let level3 = level2.create_child();
          level3.enter(|| {
            // Should see level2's value, not root's
            assert_eq!(Context::get::<String>(), Some(String::from("level2")));
          });
        });

        // Back at level1, should still see root's value
        assert_eq!(Context::get::<String>(), Some(String::from("root")));
      });
    });
  }

  #[test]
  fn child_created_via_set_scope_inherits_context() {
    let parent = Scope::new();
    parent.enter(|| {
      Context::provide(42i32);

      // Simulate what happens when a signal is created - it calls set_scope
      let child_id = Id::next();
      child_id.set_scope();

      // The child_id should now have parent as its parent
      RUNTIME.with(|runtime| {
        let entities = runtime.entities.borrow();
        assert_eq!(
          entities.get(&child_id).and_then(|entity| entity.parent),
          Some(parent.0)
        );
      });
    });
  }

  #[test]
  fn orphan_scope_has_no_parent_context() {
    // Scope::new() creates a scope with no parent
    let orphan = Scope::new();
    orphan.enter(|| {
      // No parent, so no inherited context
      assert_eq!(Context::get::<i32>(), None);

      // But can still provide its own context
      Context::provide(42i32);
      assert_eq!(Context::get::<i32>(), Some(42));
    });
  }

  #[test]
  fn newtype_wrappers_are_distinct_types() {
    #[derive(Debug, Clone, PartialEq)]
    struct UserId(i32);

    #[derive(Debug, Clone, PartialEq)]
    struct PostId(i32);

    let scope = Scope::new();
    scope.enter(|| {
      Context::provide(UserId(1));
      Context::provide(PostId(2));

      // Same underlying type (i32) but different wrapper types
      assert_eq!(Context::get::<UserId>(), Some(UserId(1)));
      assert_eq!(Context::get::<PostId>(), Some(PostId(2)));

      // Raw i32 is not provided
      assert_eq!(Context::get::<i32>(), None);
    });
  }

  #[test]
  fn context_with_rw_signal() {
    let scope = Scope::new();
    scope.enter(|| {
      let signal = RwSignal::new(42);
      Context::provide(signal);

      // Retrieve the signal from context
      let retrieved = Context::get::<RwSignal<i32>>().unwrap();
      assert_eq!(retrieved.get_untracked(), 42);

      // Modifying the retrieved signal should affect the original
      // (they're the same signal, just cloned handle)
      retrieved.set(100);
      assert_eq!(signal.get_untracked(), 100);
    });
  }

  #[test]
  fn dispose_middle_of_hierarchy() {
    let root = Scope::new();
    let middle = root.create_child();
    let leaf = middle.create_child();

    root.enter(|| Context::provide(String::from("root")));
    middle.enter(|| Context::provide(String::from("middle")));
    leaf.enter(|| Context::provide(String::from("leaf")));

    // Verify all contexts exist
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&root.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&middle.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&leaf.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
    });

    // Dispose middle - should clean up leaf too
    middle.dispose();

    RUNTIME.with(|runtime| {
      // Root should still exist
      assert!(
        runtime
          .entities
          .borrow()
          .get(&root.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
      // Middle and leaf should be gone
      assert!(
        runtime
          .entities
          .borrow()
          .get(&middle.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&leaf.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      // Parent tracking for leaf should be gone
      assert!(
        runtime
          .entities
          .borrow()
          .get(&leaf.0)
          .and_then(|entity| entity.parent)
          .is_none()
      );
    });

    // Root context still works
    root.enter(|| {
      assert_eq!(Context::get::<String>(), Some(String::from("root")));
    });
  }

  #[test]
  fn create_child_outside_enter() {
    let parent = Scope::new();

    // Provide context while inside parent
    parent.enter(|| {
      Context::provide(42i32);
    });

    // Create child outside of enter - should still establish parent relationship
    let child = parent.create_child();

    // Child should be able to see parent's context
    child.enter(|| {
      assert_eq!(Context::get::<i32>(), Some(42));
    });
  }

  #[test]
  fn context_not_visible_to_parent() {
    let parent = Scope::new();
    let child = parent.create_child();

    // Child provides context
    child.enter(|| {
      Context::provide(100i32);
    });

    // Parent should NOT see child's context
    parent.enter(|| {
      assert_eq!(Context::get::<i32>(), None);
    });
  }

  #[test]
  fn scope_reentry_preserves_context() {
    let scope = Scope::new();

    scope.enter(|| {
      Context::provide(42i32);
    });

    // Re-enter the same scope
    scope.enter(|| {
      // Context should still be there
      assert_eq!(Context::get::<i32>(), Some(42));

      // Can update it
      Context::provide(100i32);
    });

    // And it persists
    scope.enter(|| {
      assert_eq!(Context::get::<i32>(), Some(100));
    });
  }

  #[test]
  fn nested_enter_same_scope() {
    let scope = Scope::new();

    scope.enter(|| {
      Context::provide(42i32);

      // Nested enter of the same scope
      scope.enter(|| {
        assert_eq!(Context::get::<i32>(), Some(42));
        Context::provide(100i32);
      });

      // After nested enter returns, we're back in the first enter
      // The context was updated
      assert_eq!(Context::get::<i32>(), Some(100));
    });
  }

  #[test]
  fn context_with_enter_child() {
    let parent = Scope::new();

    parent.enter(|| {
      Context::provide(42i32);

      // enter_child creates a child scope and wraps a closure
      let make_child = parent.enter_child(|multiplier: i32| {
        // Should see parent's context
        let val = Context::get::<i32>().unwrap();
        // Can provide own context
        Context::provide(String::from("child"));
        val * multiplier
      });

      // Call the wrapped closure
      let (result, child_scope) = make_child(2);
      assert_eq!(result, 84);

      // Child scope has its own context
      child_scope.enter(|| {
        assert_eq!(Context::get::<String>(), Some(String::from("child")));
        // But still inherits parent's i32
        assert_eq!(Context::get::<i32>(), Some(42));
      });

      // Parent doesn't see child's String context
      assert_eq!(Context::get::<String>(), None);
    });
  }

  #[test]
  fn context_visible_in_effect() {
    let scope = Scope::new();
    let seen_value = Rc::new(Cell::new(0i32));

    scope.enter(|| {
      Context::provide(42i32);
      let trigger = RwSignal::new(0);

      let seen = seen_value.clone();
      Effect::new(move |_| {
        trigger.get(); // Subscribe to trigger
        // Effect should see the context from the scope it was created in
        if let Some(val) = Context::get::<i32>() {
          seen.set(val);
        }
      });
    });

    // Effect runs immediately on creation
    assert_eq!(seen_value.get(), 42);
  }

  #[test]
  fn zero_sized_type_as_context_marker() {
    // ZST marker types are a common pattern
    #[derive(Debug, Clone, PartialEq)]
    struct DarkMode;

    #[derive(Debug, Clone, PartialEq)]
    struct DebugEnabled;

    let scope = Scope::new();
    scope.enter(|| {
      // Provide marker
      Context::provide(DarkMode);

      // Check presence
      assert!(Context::get::<DarkMode>().is_some());
      assert!(Context::get::<DebugEnabled>().is_none());

      let child = scope.create_child();
      child.enter(|| {
        // Child inherits marker
        assert!(Context::get::<DarkMode>().is_some());

        // Child can add its own marker
        Context::provide(DebugEnabled);
        assert!(Context::get::<DebugEnabled>().is_some());
      });

      // Parent doesn't see child's marker
      assert!(Context::get::<DebugEnabled>().is_none());
    });
  }

  // Tests for the new Context struct API
  #[test]
  fn context_struct_provide_and_get() {
    let scope = Scope::new();
    scope.enter(|| {
      Context::provide(42i32);
      Context::provide(String::from("hello"));

      assert_eq!(Context::get::<i32>(), Some(42));
      assert_eq!(Context::get::<String>(), Some(String::from("hello")));
      assert_eq!(Context::get::<f64>(), None);
    });
  }

  #[test]
  fn context_struct_inheritance() {
    let parent = Scope::new();
    parent.enter(|| {
      Context::provide(42i32);

      let child = parent.create_child();
      child.enter(|| {
        // Child sees parent's context
        assert_eq!(Context::get::<i32>(), Some(42));

        // Child can shadow
        Context::provide(100i32);
        assert_eq!(Context::get::<i32>(), Some(100));
      });

      // Parent still has original
      assert_eq!(Context::get::<i32>(), Some(42));
    });
  }

  // Tests for Scope::provide_context and Scope::get_context
  #[test]
  fn scope_provide_and_get_context() {
    let scope = Scope::new();
    scope.provide_context(42i32);
    scope.provide_context(String::from("hello"));

    assert_eq!(scope.get_context::<i32>(), Some(42));
    assert_eq!(scope.get_context::<String>(), Some(String::from("hello")));
    assert_eq!(scope.get_context::<f64>(), None);
  }

  #[test]
  fn scope_context_inheritance() {
    let parent = Scope::new();
    parent.provide_context(42i32);

    let child = parent.create_child();
    // Child should see parent's context
    assert_eq!(child.get_context::<i32>(), Some(42));

    // Child can provide its own
    child.provide_context(100i32);
    assert_eq!(child.get_context::<i32>(), Some(100));

    // Parent still has original
    assert_eq!(parent.get_context::<i32>(), Some(42));
  }

  #[test]
  fn scope_context_not_visible_to_parent() {
    let parent = Scope::new();
    let child = parent.create_child();

    child.provide_context(100i32);

    // Parent should NOT see child's context
    assert_eq!(parent.get_context::<i32>(), None);
    assert_eq!(child.get_context::<i32>(), Some(100));
  }

  #[test]
  fn dispose_cleans_up_multiple_children() {
    let parent = Scope::new();
    let child1 = parent.create_child();
    let child2 = parent.create_child();
    let child3 = parent.create_child();

    parent.provide_context(String::from("parent"));
    child1.provide_context(String::from("child1"));
    child2.provide_context(String::from("child2"));
    child3.provide_context(String::from("child3"));

    // Verify all contexts exist
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&parent.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child1.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child2.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child3.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
    });

    // Dispose parent - should clean up all children
    parent.dispose();

    RUNTIME.with(|runtime| {
      // All should be gone
      assert!(
        runtime
          .entities
          .borrow()
          .get(&parent.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child1.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child2.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child3.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      // Parent tracking should be gone for all children
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child1.0)
          .and_then(|entity| entity.parent)
          .is_none()
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child2.0)
          .and_then(|entity| entity.parent)
          .is_none()
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child3.0)
          .and_then(|entity| entity.parent)
          .is_none()
      );
    });
  }

  #[test]
  fn dispose_parent_without_context_cleans_children_context() {
    let parent = Scope::new();
    let child1 = parent.create_child();
    let child2 = parent.create_child();

    // Parent has NO context, but children do
    child1.provide_context(String::from("child1"));
    child2.provide_context(String::from("child2"));

    // Verify children's contexts exist
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&parent.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child1.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child2.0)
          .is_some_and(|entity| entity.contexts.is_some())
      );
    });

    // Dispose parent - should still clean up children's contexts
    parent.dispose();

    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child1.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child2.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child1.0)
          .and_then(|entity| entity.parent)
          .is_none()
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child2.0)
          .and_then(|entity| entity.parent)
          .is_none()
      );
    });
  }

  #[test]
  fn double_dispose_is_idempotent() {
    let scope = Scope::new();
    let child = scope.create_child();

    scope.provide_context(42i32);
    child.provide_context(100i32);

    // First dispose
    scope.dispose();

    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&scope.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
    });

    // Second dispose - should not panic
    scope.dispose();
    child.dispose();

    // Still clean
    RUNTIME.with(|runtime| {
      assert!(
        runtime
          .entities
          .borrow()
          .get(&scope.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
      assert!(
        runtime
          .entities
          .borrow()
          .get(&child.0)
          .is_none_or(|entity| entity.contexts.is_none())
      );
    });
  }
}
