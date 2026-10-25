//! The derives of a facade's view over the foundation's: `Wraps`, a tuple struct of one
//! field, beside any `PhantomData` markers, holding that field as its whole content, and
//! `View`, that struct building as the view it holds for its facade's `Active`, or as
//! the core that view is thin over where `#[view(core = …)]` names it.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, TokenStream, TokenTree};
use quote::{ToTokens, format_ident, quote};
use syn::{Data, DeriveInput, Fields, GenericParam, Type, parse_quote};

/// The path to the foundation from the deriving crate: the crate itself (and its doctests),
/// through the name it gives itself, or its direct dependency.
fn foundation() -> Option<TokenStream> {
  match crate_name("skene-foundation").ok()? {
    FoundCrate::Itself => Some(quote!(::skene_foundation)),
    FoundCrate::Name(name) => {
      let name = format_ident!("{name}");

      Some(quote!(::#name))
    }
  }
}

/// The path to `Wraps` from the deriving crate: the foundation's where it is a direct
/// dependency or the crate itself, else the facade's re-export.
fn wraps() -> TokenStream {
  if let Some(foundation) = foundation() {
    return quote!(#foundation::Wraps);
  }

  for facade in ["skene-web", "skene-terminal"] {
    if let Ok(FoundCrate::Name(name)) = crate_name(facade) {
      let name = format_ident!("{name}");

      return quote!(::#name::Wraps);
    }
  }

  quote!(::skene_foundation::Wraps)
}

/// The first field of the tuple struct `input` describes, with how many `PhantomData`
/// markers follow it, the only other fields it may have.
fn field(input: &DeriveInput, derive: &str) -> syn::Result<(Type, usize)> {
  let message =
    format!("`{derive}` derives for a tuple struct of one field, beside any `PhantomData` markers");
  let Data::Struct(data) = &input.data
  else {
    return Err(syn::Error::new_spanned(input, message));
  };
  let Fields::Unnamed(fields) = &data.fields
  else {
    return Err(syn::Error::new_spanned(&data.fields, message));
  };

  let mut types = fields.unnamed.iter().map(|field| &field.ty);
  let Some(inner) = types.next()
  else {
    return Err(syn::Error::new_spanned(fields, message));
  };
  let mut markers = 0;

  for marker in types {
    let Type::Path(path) = marker
    else {
      return Err(syn::Error::new_spanned(marker, message));
    };

    if path
      .path
      .segments
      .last()
      .is_none_or(|segment| segment.ident != "PhantomData")
    {
      return Err(syn::Error::new_spanned(marker, message));
    }

    markers += 1;
  }

  Ok((inner.clone(), markers))
}

/// Expands `#[derive(Wraps)]` on `input`.
pub(crate) fn derive_wraps(input: &DeriveInput) -> syn::Result<TokenStream> {
  let (inner, markers) = field(input, "Wraps")?;
  let name = &input.ident;
  let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
  let wraps = wraps();
  let markers = (0..markers).map(|_| quote!(::core::marker::PhantomData));

  Ok(quote! {
    impl #impl_generics #wraps for #name #type_generics #where_clause {
      type Inner = #inner;

      #[inline(always)]
      fn wrap(inner: Self::Inner) -> Self {
        Self(inner #(, #markers)*)
      }

      #[inline(always)]
      fn inner(&self) -> &Self::Inner {
        &self.0
      }

      #[inline(always)]
      fn inner_mut(&mut self) -> &mut Self::Inner {
        &mut self.0
      }

      #[inline(always)]
      fn into_inner(self) -> Self::Inner {
        self.0
      }
    }
  })
}

/// The facades a derived view builds for: the path to each one's `Active` from the
/// deriving crate, the crate itself (and its doctests) through the name it gives itself or
/// its dependency, with the platform its build names.
fn facades() -> Vec<(TokenStream, &'static str)> {
  [("skene-web", "web"), ("skene-terminal", "terminal")]
    .into_iter()
    .filter_map(|(facade, platform)| {
      let name = match crate_name(facade).ok()? {
        FoundCrate::Itself => format_ident!("{}", facade.replace('-', "_")),
        FoundCrate::Name(name) => format_ident!("{name}"),
      };

      Some((quote!(::#name::Active), platform))
    })
    .collect()
}

/// Whether `tokens` name any of `names`, at any depth.
fn names_any(tokens: TokenStream, names: &[Ident]) -> bool {
  tokens.into_iter().any(|token| match token {
    TokenTree::Ident(ident) => names.contains(&ident),
    TokenTree::Group(group) => names_any(group.stream(), names),
    TokenTree::Punct(_) | TokenTree::Literal(_) => false,
  })
}

/// The core `#[view(core = …)]` names on `input`, if any.
fn core(input: &DeriveInput) -> syn::Result<Option<Type>> {
  let mut core = None;

  for attribute in &input.attrs {
    if !attribute.path().is_ident("view") {
      continue;
    }

    attribute.parse_nested_meta(|meta| {
      if meta.path.is_ident("core") {
        core = Some(meta.value()?.parse()?);

        Ok(())
      }
      else {
        Err(meta.error("`#[view]` takes `core = <the core the view is thin over>`"))
      }
    })?;
  }

  Ok(core)
}

/// Expands `#[derive(View)]` on `input`.
pub(crate) fn derive_view(input: &DeriveInput) -> syn::Result<TokenStream> {
  view(input, facades())
}

/// Expands `#[derive(View)]` on `input` for `facades`, the path to each one's `Active`
/// with the platform its build names.
fn view(
  input: &DeriveInput,
  facades: Vec<(TokenStream, &'static str)>,
) -> syn::Result<TokenStream> {
  let (inner, _) = field(input, "View")?;
  let core = core(input)?;
  let name = &input.ident;
  let foundation = foundation().unwrap_or_else(|| quote!(::skene_foundation));

  if facades.is_empty() {
    return Err(syn::Error::new_spanned(
      name,
      "a derived view belongs to one facade: `View` derives for the `Active` of the facade \
       (`skene-web` or `skene-terminal`) the deriving crate is or depends on, and this \
       crate reaches neither",
    ));
  }

  let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
  let params: Vec<Ident> = input
    .generics
    .params
    .iter()
    .map(|param| match param {
      GenericParam::Type(param) => param.ident.clone(),
      GenericParam::Const(param) => param.ident.clone(),
      GenericParam::Lifetime(param) => param.lifetime.ident.clone(),
    })
    .collect();
  let generic = names_any(inner.to_token_stream(), &params);
  let gated = facades.len() > 1;
  let views = facades.into_iter().map(|(active, platform)| {
    let gate = gated.then(|| quote!(#[cfg(skene_platform = #platform)]));
    let mut generics = input.generics.clone();

    if let Some(core) = &core {
      generics
        .make_where_clause()
        .predicates
        .extend::<[syn::WherePredicate; 2]>([
          parse_quote!(#inner: #foundation::Thin<#active, Core = #core>),
          parse_quote!(Self: 'static),
        ]);
    }
    else if generic {
      generics
        .make_where_clause()
        .predicates
        .extend::<[syn::WherePredicate; 2]>([
          parse_quote!(#inner: #foundation::View<#active>),
          parse_quote!(Self: 'static),
        ]);
    }
    else if !params.is_empty() {
      generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: 'static));
    }

    let (_, _, view_where_clause) = generics.split_for_impl();
    let body = if let Some(core) = &core {
      quote! {
        fn build(core: #core) -> #foundation::NodeId {
          #foundation::View::<#active>::build(core)
        }

        build(#foundation::Thin::<#active>::into_core(self.0))
      }
    }
    else if params.is_empty() || generic {
      quote!(#foundation::View::<#active>::build(self.0))
    }
    else {
      quote! {
        fn build(inner: #inner) -> #foundation::NodeId {
          #foundation::View::<#active>::build(inner)
        }

        build(self.0)
      }
    };

    quote! {
      #gate
      impl #impl_generics #foundation::View<#active> for #name #type_generics #view_where_clause {
        fn build(self) -> #foundation::NodeId {
          #body
        }
      }
    }
  });

  Ok(quote! {
    impl #impl_generics #foundation::AnyKind for #name #type_generics #where_clause {}

    #(#views)*
  })
}

#[cfg(test)]
mod tests {
  use quote::{format_ident, quote};
  use syn::{DeriveInput, parse_quote};

  use super::view;

  /// The expansion of `#[derive(View)]` on `input` for `facades`, as a string.
  fn expand(input: &DeriveInput, facades: &[&'static str]) -> String {
    let facades = facades
      .iter()
      .map(|&platform| {
        let facade = format_ident!("skene_{platform}");

        (quote!(::#facade::Active), platform)
      })
      .collect();

    view(input, facades)
      .expect("the derive expands")
      .to_string()
  }

  /// Its expansion with every blank removed, so a test compares the tokens alone.
  fn compact(expansion: &str) -> String {
    expansion.split_whitespace().collect()
  }

  #[test]
  fn a_crate_reaching_no_facade_is_refused() {
    let input: DeriveInput = parse_quote!(
      struct Button(Inner);
    );
    let error = view(&input, Vec::new()).expect_err("no facade is reached");

    assert!(
      error
        .to_string()
        .starts_with("a derived view belongs to one facade"),
      "{error}"
    );
  }

  #[test]
  fn a_view_without_parameters_builds_for_its_facade_alone() {
    let expansion = compact(&expand(
      &parse_quote!(
        struct Button(Inner);
      ),
      &["web"],
    ));

    assert!(
      expansion.contains("impl::skene_foundation::View<::skene_web::Active>forButton{"),
      "{expansion}"
    );
    assert!(!expansion.contains("inline"), "{expansion}");
    assert!(!expansion.contains("cfg"), "{expansion}");
    assert!(!expansion.contains("Realization"), "{expansion}");
  }

  #[test]
  fn a_field_naming_no_parameter_builds_through_a_function_that_is_not_generic() {
    let input = parse_quote!(
      struct Group<T>(Core, PhantomData<T>);
    );
    let expansion = compact(&expand(&input, &["terminal"]));

    assert!(
      expansion.contains(
        "impl<T>::skene_foundation::View<::skene_terminal::Active>forGroup<T>whereSelf:'static{"
      ),
      "{expansion}"
    );
    assert!(
      expansion.contains("fnbuild(inner:Core)->::skene_foundation::NodeId{"),
      "{expansion}"
    );
    assert!(expansion.contains("build(self.0)"), "{expansion}");
  }

  #[test]
  fn a_field_generic_in_its_parameters_builds_generic_in_them_alone() {
    let input = parse_quote!(
      struct Theme<S>(Core<S>);
    );
    let expansion = compact(&expand(&input, &["web"]));

    assert!(
      expansion.contains(
        "impl<S>::skene_foundation::View<::skene_web::Active>forTheme<S>whereCore<S>:::\
         skene_foundation::View<::skene_web::Active>,Self:'static{"
      ),
      "{expansion}"
    );
    assert!(!expansion.contains("fnbuild(inner"), "{expansion}");
  }

  #[test]
  fn a_view_naming_its_core_builds_the_core_through_a_function_that_is_not_generic() {
    let input = parse_quote!(
      #[view(core = ControlCore<Active>)]
      struct Control<B: Behavior>(Core<B>);
    );
    let expansion = compact(&expand(&input, &["web"]));

    assert!(
      expansion.contains(
        "impl<B:Behavior>::skene_foundation::View<::skene_web::Active>forControl<B>whereCore<B>\
         :::skene_foundation::Thin<::skene_web::Active,Core=ControlCore<Active>>,Self:'static{"
      ),
      "{expansion}"
    );
    assert!(
      expansion.contains("fnbuild(core:ControlCore<Active>)->::skene_foundation::NodeId{"),
      "{expansion}"
    );
    assert!(
      expansion
        .contains("build(::skene_foundation::Thin::<::skene_web::Active>::into_core(self.0))"),
      "{expansion}"
    );
  }

  #[test]
  fn a_view_attribute_naming_anything_but_its_core_is_refused() {
    let input: DeriveInput = parse_quote!(
      #[view(shape = Core)]
      struct Control<B>(Core<B>);
    );
    let error = view(&input, Vec::new()).expect_err("the attribute is refused");

    assert!(error.to_string().starts_with("`#[view]` takes"), "{error}");
  }

  #[test]
  fn a_crate_reaching_both_facades_builds_for_each_under_its_platform() {
    let expansion = compact(&expand(
      &parse_quote!(
        struct Card(Inner);
      ),
      &["web", "terminal"],
    ));

    assert!(
      expansion.contains(
        "#[cfg(skene_platform=\"web\")]impl::skene_foundation::View<::skene_web::Active>"
      ),
      "{expansion}"
    );
    assert!(
      expansion.contains(
        "#[cfg(skene_platform=\"terminal\")]impl::skene_foundation::View<::skene_terminal::\
         Active>"
      ),
      "{expansion}"
    );
  }
}
