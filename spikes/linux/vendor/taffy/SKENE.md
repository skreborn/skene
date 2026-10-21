Skene's copy of `taffy` 0.15.0 (crates.io), carried until the change is offered upstream or
a taffy release that has it is taken.

Skene's change, marked `Skene:` in the source, measures a child only where a step reads what
it measures:
- compute/flexbox.rs: in a single-line flex container of definite cross size, a stretched
  item of `auto` cross size has the line's cross size for its hypothetical cross size, which
  the stretch gives it in any case, and is not measured.
- Cargo.toml: the examples and the dev-dependencies are left out.

Two changes Skene carried on 0.14.0 are taffy's own in 0.15.0 and are no longer carried: a grid
item's min-content and max-content contributions measured only where a track it spans uses
them (`GridItem::spans_track_matching`), and a flex container asked for its main size alone
returning once it is known.

The licence file (LICENSE, MIT) is copied from the taffy repository at its v0.15.0 tag, since
the published crate ships none.
