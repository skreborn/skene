//! How a link opens and what it discloses to its destination.

use skene_foundation::{NodeFacts, NodeId, NodeSpec, RunFacts};

use crate::Active;

/// How a link opens and what it discloses to its destination.
///
/// The default is the most private behavior: an external destination (an absolute URL,
/// one naming a host) opens on a new page that receives neither a handle back to ours
/// nor the address it came from; a relative destination opens in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[must_use]
pub struct LinkPolicy {
  /// Whether the link opens a new page; unset, an external destination does and a
  /// relative one opens in place.
  pub new_page: Option<bool>,
  /// Whether the opened page receives a handle back to ours.
  pub expose_opener: bool,
  /// Whether an external navigation discloses the page it came from.
  pub send_referrer: bool,
  /// The endorsement the destination gets from crawlers, when it needs one.
  pub annotation: Option<LinkAnnotation>,
}

impl LinkPolicy {
  /// Creates the default policy.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets whether the link opens a new page, overriding the external/relative default.
  #[inline(always)]
  pub const fn set_new_page(mut self, new_page: bool) -> Self {
    self.new_page = Some(new_page);

    self
  }

  /// Lets the opened page reach back to ours.
  #[inline(always)]
  pub const fn set_expose_opener(mut self, expose_opener: bool) -> Self {
    self.expose_opener = expose_opener;

    self
  }

  /// Lets an external navigation disclose the page it came from.
  #[inline(always)]
  pub const fn set_send_referrer(mut self, send_referrer: bool) -> Self {
    self.send_referrer = send_referrer;

    self
  }

  /// Sets the endorsement the destination gets from crawlers.
  #[inline(always)]
  pub const fn set_annotation(mut self, annotation: LinkAnnotation) -> Self {
    self.annotation = Some(annotation);

    self
  }

  /// The attributes of a link to `href` under this policy: `href`, `target` and `rel`,
  /// with no `href` where following it would run script.
  pub(crate) fn attributes(self, href: &str) -> Vec<(&'static str, String)> {
    let external = crate::markup::leaves_origin(href);
    let new_page = self.new_page.unwrap_or(external);
    let mut rel = Vec::new();
    let mut attributes = Vec::new();

    if !crate::markup::runs_script(href) {
      attributes.push(("href", href.to_owned()));
    }

    if new_page && !self.expose_opener {
      rel.push("noopener");
    }

    if external && !self.send_referrer {
      rel.push("noreferrer");
    }

    match self.annotation {
      Some(LinkAnnotation::NoFollow) => rel.push("nofollow"),
      Some(LinkAnnotation::Sponsored) => rel.push("sponsored"),
      Some(LinkAnnotation::Ugc) => rel.push("ugc"),
      None => {}
    }

    if new_page {
      attributes.push(("target", "_blank".to_owned()));
    }

    if !rel.is_empty() {
      attributes.push(("rel", rel.join(" ")));
    }

    attributes
  }

  /// Rewrites the attributes of the link `spec` realized at `node`, given the default
  /// policy's, as this policy gives them.
  pub(crate) fn realize(self, node: NodeId, spec: &NodeSpec) {
    use crate::low_level::WebMarkup;

    let NodeFacts::Run(RunFacts {
      link: Some(href), ..
    }) = &spec.facts
    else {
      return;
    };
    let attributes = self.attributes(href);

    for (name, _) in Self::default().attributes(href) {
      if attributes.iter().all(|(kept, _)| *kept != name) {
        Active::remove_attribute(node, name);
      }
    }

    for (name, value) in attributes {
      Active::set_attribute(node, name, &value);
    }
  }
}

/// A link's crawler endorsement; the qualifiers are mutually exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum LinkAnnotation {
  /// The destination is not endorsed.
  NoFollow,
  /// The destination is a paid placement.
  Sponsored,
  /// The destination came from user-generated content.
  Ugc,
}
