//! Semantic roles: AccessKit's vocabulary.

/// The semantic role of a box, beyond a plain grouping container: AccessKit's role
/// vocabulary, open to every role an accessibility layer knows.
///
/// A box with no role carries no semantic meaning. Each platform maps a role to
/// whatever its accessibility layer understands—a semantic element or an ARIA role on
/// the web, an AccessKit node on a native platform. A landmark role describes a region
/// of a document; a widget role describes a control a view draws itself, and pairs with
/// the [`AccessProp`](crate::AccessProp) state that control reports.
pub use accesskit::Role;
