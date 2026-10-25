//! The derives of a facade's view over the foundation's: `Wraps`, a tuple struct of one
//! field, beside any `PhantomData` markers, holding that field as its whole content, and
//! `View`, that struct building as the view it holds for its facade's `Active`.

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

/// Expands `#[derive(View)]` on `input`.
pub(crate) fn derive_view(input: &DeriveInput) -> syn::Result<TokenStream> {
  let (inner, _) = field(input, "View")?;
  let name = &input.ident;
  let foundation = foundation().unwrap_or_else(|| quote!(::skene_foundation));
  let facades = facades();

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

    if generic {
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
    let body = if params.is_empty() || generic {
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
