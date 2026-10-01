//! The `Wraps` derive: a tuple struct of one field holding that field as its whole
//! content.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields};

/// The path to `Wraps` from the deriving crate: the foundation's where it is a direct
/// dependency or the crate itself, else the facade's re-export.
fn wraps() -> TokenStream {
  if let Ok(found) = crate_name("skene-foundation") {
    return match found {
      FoundCrate::Itself => quote!(crate::Wraps),
      FoundCrate::Name(name) => {
        let name = format_ident!("{name}");

        quote!(::#name::Wraps)
      }
    };
  }

  for facade in ["skene-web", "skene-terminal"] {
    if let Ok(FoundCrate::Name(name)) = crate_name(facade) {
      let name = format_ident!("{name}");

      return quote!(::#name::Wraps);
    }
  }

  quote!(::skene_foundation::Wraps)
}

/// Expands `#[derive(Wraps)]` on `input`.
pub(crate) fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
  let Data::Struct(data) = &input.data
  else {
    return Err(syn::Error::new_spanned(
      input,
      "`Wraps` derives for a tuple struct of one field",
    ));
  };
  let Fields::Unnamed(fields) = &data.fields
  else {
    return Err(syn::Error::new_spanned(
      &data.fields,
      "`Wraps` derives for a tuple struct of one field",
    ));
  };

  if fields.unnamed.len() != 1 {
    return Err(syn::Error::new_spanned(
      fields,
      "`Wraps` derives for a tuple struct of exactly one field",
    ));
  }

  let inner = &fields.unnamed[0].ty;
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
