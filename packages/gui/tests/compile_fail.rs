//! The rules the types enforce, each a program that must not compile, with the
//! diagnostic it raises beside it: layout's static constraints, children last, the
//! expression kinds and the derives.
//!
//! `TRYBUILD=overwrite` rewrites the diagnostics once a change to one is meant.

#![cfg(skene_leaf = "host")]

#[test]
fn what_the_rules_forbid_does_not_compile() {
  trybuild::TestCases::new().compile_fail("tests/compile_fail/*.rs");
}
