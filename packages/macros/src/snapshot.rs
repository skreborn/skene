//! The `Snapshot` derive: a struct records its fields by key, an enum its variant by
//! key with the value it carries.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitStr, ext::IdentExt};

use crate::store::reactive;

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
  /// Each recorded field's label and the name its key is taken from.
  keys: Vec<(String, String)>,
}

/// Asserts at compile time that no two of `keys`, labels beside the names their keys are
/// taken from, share a key.
fn collisions(keys: &[(String, String)], reactive: &TokenStream) -> TokenStream {
  let asserts = keys.iter().enumerate().flat_map(|(index, (label, name))| {
    keys[index + 1..]
      .iter()
      .map(move |(other_label, other_name)| {
        let message =
          format!("the snapshot keys of `{label}` and `{other_label}` collide; tag one of them");

        quote! {
          const _: () = ::core::assert!(
            #reactive::field_key(#name) != #reactive::field_key(#other_name),
            #message
          );
        }
      })
  });

  quote!(#(#asserts)*)
}

/// The code for `fields`, reading each through `access` (`self.name`, or a binding).
fn field_code(fields: &Fields, reactive: &TokenStream, bound: bool) -> syn::Result<FieldCode> {
  let mut record = Vec::new();
  let mut restore = Vec::new();
  let mut absent = Vec::new();
  let mut bindings = Vec::new();
  let mut keys = Vec::new();
  let named = matches!(fields, Fields::Named(_));

  for (position, field) in fields.iter().enumerate() {
    let attributes = field_attributes(&field.attrs)?;
    let ty = &field.ty;
    let label = field
      .ident
      .as_ref()
      .map_or_else(|| position.to_string(), |ident| ident.unraw().to_string());
    let name = attributes.tag.as_deref().unwrap_or(&label);
    let key = quote!(const { #reactive::field_key(#name) });
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
      keys.push((label.clone(), name.to_owned()));
    }

    let restored = if attributes.skip {
      quote!(<#ty as ::core::default::Default>::default())
    }
    else {
      quote! {
        match __record.field(#key) {
          ::core::option::Option::Some(__value) => <#ty as #reactive::Snapshot>::restore(__value)
            .or_else(<#ty as #reactive::Snapshot>::absent)?,
          ::core::option::Option::None => <#ty as #reactive::Snapshot>::absent()?,
        }
      }
    };
    let absent_value = if attributes.skip {
      quote!(<#ty as ::core::default::Default>::default())
    }
    else {
      quote!(<#ty as #reactive::Snapshot>::absent()?)
    };

    restore.push(quote!(#target #restored,));
    absent.push(quote!(#target #absent_value,));
  }

  Ok(FieldCode {
    record: quote!(#(#record)*),
    restore: quote!(#(#restore)*),
    absent: quote!(#(#absent)*),
    bindings,
    named,
    keys,
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
#[expect(
  clippy::too_many_lines,
  reason = "one expansion start to end; length is the generated surface, not complexity"
)]
pub(crate) fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
  let name = &input.ident;
  let reactive = reactive();
  let mut generics = input.generics.clone();

  for parameter in generics.type_params_mut() {
    parameter
      .bounds
      .push(syn::parse_quote!(#reactive::Snapshot));
  }

  let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
  let mut checks = Vec::new();
  let body = match &input.data {
    Data::Struct(data) => {
      let code = field_code(&data.fields, &reactive, false)?;

      checks.push(collisions(&code.keys, &reactive));

      let record = &code.record;
      let restore = construct(&quote!(Self), &data.fields, &code.restore);
      let absent = construct(&quote!(Self), &data.fields, &code.absent);

      quote! {
        fn record(&self, __recorder: &mut #reactive::Recorder) {
          __recorder.record(|__fields| { #record });
        }

        fn restore(__value: #reactive::Value<'_>) -> ::core::option::Option<Self> {
          let #reactive::Value::Record(__record) = __value
          else {
            return ::core::option::Option::None;
          };

          ::core::option::Option::Some(#restore)
        }

        fn absent() -> ::core::option::Option<Self> {
          ::core::option::Option::Some(#absent)
        }
      }
    }
    Data::Enum(data) => {
      let mut record_arms = Vec::new();
      let mut restore_arms = Vec::new();
      let mut restore_constants = Vec::new();
      let mut default_variant = None;
      let mut variant_keys = Vec::new();

      for variant in &data.variants {
        let (default, tag) = variant_attributes(&variant.attrs)?;
        let ident = &variant.ident;
        let label = ident.unraw().to_string();
        let name = tag.as_deref().unwrap_or(&label);
        let key = quote!(const { #reactive::field_key(#name) });
        let constant = format_ident!("__VARIANT_KEY_{}", restore_constants.len());
        let code = field_code(&variant.fields, &reactive, true)?;

        checks.push(collisions(&code.keys, &reactive));
        variant_keys.push((label.clone(), name.to_owned()));

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
          quote!(::core::option::Option::Some(#path))
        }
        else {
          quote! {
            {
              let #reactive::Value::Record(__record) = __payload
              else {
                return ::core::option::Option::None;
              };

              ::core::option::Option::Some(#restore)
            }
          }
        };

        record_arms.push(quote! {
          #pattern => __recorder.variant(#key, |__recorder| { #payload }),
        });
        restore_constants.push(quote!(const #constant: u32 = #reactive::field_key(#name);));
        restore_arms.push(quote!(#constant => #restored,));

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

      checks.push(collisions(&variant_keys, &reactive));

      let fallback = default_variant.as_ref().map_or_else(
        || quote!(::core::option::Option::None),
        |ident| quote!(::core::option::Option::Some(Self::#ident)),
      );
      let absent = fallback.clone();

      quote! {
        fn record(&self, __recorder: &mut #reactive::Recorder) {
          match self {
            #(#record_arms)*
          }
        }

        fn restore(__value: #reactive::Value<'_>) -> ::core::option::Option<Self> {
          let #reactive::Value::Variant(__key, mut __reader) = __value
          else {
            return ::core::option::Option::None;
          };
          let __payload = __reader.value()?;

          #(#restore_constants)*

          match __key {
            #(#restore_arms)*
            _ => #fallback,
          }
        }

        fn absent() -> ::core::option::Option<Self> {
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
    #(#checks)*

    #[automatically_derived]
    impl #impl_generics #reactive::Snapshot for #name #ty_generics #where_clause {
      #body
    }
  })
}
