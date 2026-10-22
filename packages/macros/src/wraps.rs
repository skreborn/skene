//! The derives of a facade's view over the foundation's: `Wraps`, a tuple struct of one
//! field holding that field as its whole content, and `View`, that struct building as
//! the view it holds.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Type, parse_quote};

/// The path to the foundation from the deriving crate: the crate itself, or its direct
/// dependency.
fn foundation() -> Option<TokenStream> {
  match crate_name("skene-foundation").ok()? {
    FoundCrate::Itself => Some(quote!(crate)),
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

/// The one field of the tuple struct `input` describes.
fn field(input: &DeriveInput, derive: &str) -> syn::Result<Type> {
  let message = format!("`{derive}` derives for a tuple struct of one field");
  let Data::Struct(data) = &input.data
  else {
    return Err(syn::Error::new_spanned(input, message));
  };
  let Fields::Unnamed(fields) = &data.fields
  else {
    return Err(syn::Error::new_spanned(&data.fields, message));
  };

  if fields.unnamed.len() != 1 {
    return Err(syn::Error::new_spanned(fields, message));
  }

  Ok(fields.unnamed[0].ty.clone())
}

/// Expands `#[derive(Wraps)]` on `input`.
pub(crate) fn derive_wraps(input: &DeriveInput) -> syn::Result<TokenStream> {
  let inner = field(input, "Wraps")?;
  let name = &input.ident;
  let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
  let wraps = wraps();

  Ok(quote! {
    impl #impl_generics #wraps for #name #type_generics #where_clause {
      type Inner = #inner;

      #[inline(always)]
      fn wrap(inner: Self::Inner) -> Self {
        Self(inner)
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

/// Expands `#[derive(View)]` on `input`.
pub(crate) fn derive_view(input: &DeriveInput) -> syn::Result<TokenStream> {
  let inner = field(input, "View")?;
  let name = &input.ident;
  let foundation = foundation().unwrap_or_else(|| quote!(::skene_foundation));
  let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
  let mut generics = input.generics.clone();

  generics
    .params
    .push(parse_quote!(__P: #foundation::Realization));
  generics
    .make_where_clause()
    .predicates
    .extend::<[syn::WherePredicate; 2]>([
      parse_quote!(#inner: #foundation::View<__P>),
      parse_quote!(Self: 'static),
    ]);

  let (view_generics, _, view_where_clause) = generics.split_for_impl();

  Ok(quote! {
    impl #impl_generics #foundation::AnyKind for #name #type_generics #where_clause {}

    impl #view_generics #foundation::View<__P> for #name #type_generics #view_where_clause {
      #[inline(always)]
      fn build(self) -> #foundation::NodeId {
        #foundation::View::<__P>::build(self.0)
      }
    }
  })
}
