//! The `Snapshot` derive: a struct records its fields by key, an enum its variant by
//! key with the value it carries.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitStr, ext::IdentExt};

use crate::store::{field_key, reactive};

/// What a field's `#[snapshot(...)]` attributes say: whether it is skipped, and its tag.
struct FieldAttributes {
  skip: bool,
  tag: Option<String>,
}

/// Reads `#[snapshot(...)]` on a field: `skip`, `tag = "..."`.
fn field_attributes(attributes: &[syn::Attribute]) -> syn::Result<FieldAttributes> {
  let mut read = FieldAttributes {
    skip: false,
    tag: None,
  };

  for attribute in attributes {
    if !attribute.path().is_ident("snapshot") {
      continue;
    }

    attribute.parse_nested_meta(|meta| {
      if meta.path.is_ident("skip") {
        read.skip = true;

        return Ok(());
      }

      if meta.path.is_ident("tag") {
        let value: LitStr = meta.value()?.parse()?;

        read.tag = Some(value.value());

        return Ok(());
      }

      Err(meta.error("expected `skip` or `tag = \"...\"`"))
    })?;
  }

  Ok(read)
}

/// Reads `#[snapshot(...)]` on a variant: `default`, `tag = "..."`.
fn variant_attributes(attributes: &[syn::Attribute]) -> syn::Result<(bool, Option<String>)> {
  let mut default = false;
  let mut tag = None;

  for attribute in attributes {
    if !attribute.path().is_ident("snapshot") {
      continue;
    }

    attribute.parse_nested_meta(|meta| {
      if meta.path.is_ident("default") {
        default = true;

        return Ok(());
      }

      if meta.path.is_ident("tag") {
        let value: LitStr = meta.value()?.parse()?;

        tag = Some(value.value());

        return Ok(());
      }

      Err(meta.error("expected `default` or `tag = \"...\"`"))
    })?;
  }

  Ok((default, tag))
}

/// The recording and restoring of one set of fields, named or positional.
struct FieldCode {
  /// Records `self.<member>` or the bound `<binding>` under each key.
  record: TokenStream,
  /// Restores every field from the record `__record` into a constructor body.
  restore: TokenStream,
  /// Builds every field from its absent value into a constructor body.
  absent: TokenStream,
  /// The bindings a variant pattern destructures into.
  bindings: Vec<syn::Ident>,
  /// Whether the fields are named.
  named: bool,
}

/// The code for `fields`, reading each through `access` (`self.name`, or a binding).
fn field_code(fields: &Fields, reactive: &TokenStream, bound: bool) -> syn::Result<FieldCode> {
  let mut record = Vec::new();
  let mut restore = Vec::new();
  let mut absent = Vec::new();
  let mut bindings = Vec::new();
  let named = matches!(fields, Fields::Named(_));

  for (position, field) in fields.iter().enumerate() {
    let attributes = field_attributes(&field.attrs)?;
    let ty = &field.ty;
    let label = field
      .ident
      .as_ref()
      .map_or_else(|| position.to_string(), |ident| ident.unraw().to_string());
    let key = field_key(attributes.tag.as_deref().unwrap_or(&label));
    let binding = format_ident!("__field_{position}");
    let member: syn::Member = field.ident.clone().map_or_else(
      || syn::Member::Unnamed(syn::Index::from(position)),
      syn::Member::Named,
    );
    let access = if bound {
      quote!(#binding)
    }
    else {
      quote!(&self.#member)
    };
    let target = field.ident.as_ref().map(|ident| quote!(#ident:));

    bindings.push(binding);

    if !attributes.skip {
      record.push(quote! {
        __fields.field(#key, #access);
      });
    }

    let restored = if attributes.skip {
      quote!(<#ty as #reactive::Snapshot>::absent()?)
    }
    else {
      quote! {
        match __record.field(#key) {
          Some(__value) => <#ty as #reactive::Snapshot>::restore(__value)
            .or_else(<#ty as #reactive::Snapshot>::absent)?,
          None => <#ty as #reactive::Snapshot>::absent()?,
        }
      }
    };

    restore.push(quote!(#target #restored,));
    absent.push(quote!(#target <#ty as #reactive::Snapshot>::absent()?,));
  }

  Ok(FieldCode {
    record: quote!(#(#record)*),
    restore: quote!(#(#restore)*),
    absent: quote!(#(#absent)*),
    bindings,
    named,
  })
}

/// Wraps a constructor body in the braces or parentheses `fields` take.
fn construct(path: &TokenStream, fields: &Fields, body: &TokenStream) -> TokenStream {
  match fields {
    Fields::Named(_) => quote!(#path { #body }),
    Fields::Unnamed(_) => quote!(#path ( #body )),
    Fields::Unit => quote!(#path),
  }
}

/// Expands `#[derive(Snapshot)]`.
pub fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
  let name = &input.ident;
  let reactive = reactive();
  let mut generics = input.generics.clone();

  for parameter in generics.type_params_mut() {
    parameter
      .bounds
      .push(syn::parse_quote!(#reactive::Snapshot));
  }

  let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
  let body = match &input.data {
    Data::Struct(data) => {
      let code = field_code(&data.fields, &reactive, false)?;
      let record = &code.record;
      let restore = construct(&quote!(Self), &data.fields, &code.restore);
      let absent = construct(&quote!(Self), &data.fields, &code.absent);

      quote! {
        fn record(&self, __recorder: &mut #reactive::Recorder) {
          __recorder.record(|__fields| { #record });
        }

        fn restore(__value: #reactive::Value<'_>) -> Option<Self> {
          let #reactive::Value::Record(__record) = __value
          else {
            return None;
          };

          Some(#restore)
        }

        fn absent() -> Option<Self> {
          Some(#absent)
        }
      }
    }
    Data::Enum(data) => {
      let mut record_arms = Vec::new();
      let mut restore_arms = Vec::new();
      let mut default_variant = None;

      for variant in &data.variants {
        let (default, tag) = variant_attributes(&variant.attrs)?;
        let ident = &variant.ident;
        let label = ident.unraw().to_string();
        let key = field_key(tag.as_deref().unwrap_or(&label));
        let code = field_code(&variant.fields, &reactive, true)?;
        let bindings = &code.bindings;
        let path = quote!(Self::#ident);
        let pattern = match &variant.fields {
          Fields::Named(fields) => {
            let names = fields.named.iter().map(|field| &field.ident);

            quote!(#path { #(#names: #bindings),* })
          }
          Fields::Unnamed(_) => quote!(#path ( #(#bindings),* )),
          Fields::Unit => quote!(#path),
        };
        let record = &code.record;
        let payload = if code.named || !bindings.is_empty() {
          quote!(__recorder.record(|__fields| { #record });)
        }
        else {
          quote!(__recorder.unit();)
        };
        let restore = construct(&path, &variant.fields, &code.restore);
        let restored = if bindings.is_empty() {
          quote!(Some(#path))
        }
        else {
          quote! {
            {
              let #reactive::Value::Record(__record) = __payload
              else {
                return None;
              };

              Some(#restore)
            }
          }
        };

        record_arms.push(quote! {
          #pattern => __recorder.variant(#key, |__recorder| { #payload }),
        });
        restore_arms.push(quote!(#key => #restored,));

        if default {
          if !bindings.is_empty() {
            return Err(syn::Error::new_spanned(
              variant,
              "#[snapshot(default)] names a unit variant",
            ));
          }

          if default_variant.is_some() {
            return Err(syn::Error::new_spanned(
              variant,
              "#[snapshot(default)] is given once",
            ));
          }

          default_variant = Some(ident.clone());
        }
      }

      let absent = match &default_variant {
        Some(ident) => quote!(Some(Self::#ident)),
        None => quote!(None),
      };
      let fallback = match &default_variant {
        Some(ident) => quote!(Some(Self::#ident)),
        None => quote!(None),
      };

      quote! {
        fn record(&self, __recorder: &mut #reactive::Recorder) {
          match self {
            #(#record_arms)*
          }
        }

        fn restore(__value: #reactive::Value<'_>) -> Option<Self> {
          let #reactive::Value::Variant(__key, mut __reader) = __value
          else {
            return None;
          };
          let __payload = __reader.value()?;

          match __key {
            #(#restore_arms)*
            _ => #fallback,
          }
        }

        fn absent() -> Option<Self> {
          #absent
        }
      }
    }
    Data::Union(_) => {
      return Err(syn::Error::new_spanned(
        input,
        "#[derive(Snapshot)] supports structs and enums",
      ));
    }
  };

  Ok(quote! {
    #[automatically_derived]
    impl #impl_generics #reactive::Snapshot for #name #ty_generics #where_clause {
      #body
    }
  })
}
