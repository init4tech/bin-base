use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Ident, LitStr};

/// A parsed Field of a struct
pub(crate) struct Field {
    env_var: Option<LitStr>,
    field_name: Option<Ident>,
    field_type: syn::Type,

    optional: bool,
    infallible: bool,
    skip: bool,
    desc: Option<String>,

    _attrs: Vec<syn::Attribute>,

    span: proc_macro2::Span,
}

impl TryFrom<&syn::Field> for Field {
    type Error = syn::Error;

    fn try_from(field: &syn::Field) -> Result<Self, syn::Error> {
        let mut optional = false;
        let mut env_var = None;
        let mut infallible = false;
        let mut desc = None;
        let mut skip = false;

        field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("from_env"))
            .for_each(|attr| {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        skip = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("optional") {
                        optional = true;
                        return Ok(());
                    }
                    if meta.path.is_ident("var") {
                        env_var = Some(meta.value()?.parse::<LitStr>()?);
                        return Ok(());
                    }
                    if meta.path.is_ident("desc") {
                        desc = Some(meta.value()?.parse::<LitStr>()?.value());
                        return Ok(());
                    }
                    if meta.path.is_ident("infallible") {
                        infallible = true;
                    }
                    Ok(())
                });
            });

        if desc.is_none() && env_var.is_some() {
            return Err(syn::Error::new(
                field.span(),
                "Missing description for field. Use `#[from_env(desc = \"DESC\")]`",
            ));
        }

        let field_type = field.ty.clone();
        let field_name = field.ident.clone();
        let span = field.span();

        Ok(Field {
            env_var,
            field_name,
            field_type,
            optional,
            skip,
            infallible,
            desc,
            _attrs: field
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("from_env"))
                .cloned()
                .collect(),
            span,
        })
    }
}

impl Field {
    pub(crate) fn field_name(&self, idx: usize) -> Ident {
        if let Some(field_name) = self.field_name.as_ref() {
            return field_name.clone();
        }

        let n = format!("field_{idx}");
        syn::parse_str::<Ident>(&n)
            .map_err(|_| syn::Error::new(self.span, "Failed to create field name"))
            .unwrap()
    }

    /// Produces a line for the `inventory` function
    pub(crate) fn expand_env_item_info(&self) -> TokenStream {
        if self.skip {
            return quote! {};
        }

        let description = self.desc.clone().unwrap_or_default();
        let optional = self.optional;

        if let Some(env_var) = &self.env_var {
            let var_name = env_var.value();

            return quote! {
                items.push(&EnvItemInfo {
                    var: #var_name,
                    description: #description,
                    optional: #optional,
                });
            };
        }

        let field_ty = &self.field_type;
        quote! {
            items.extend(
                <#field_ty as FromEnv>::inventory()
            );
        }
    }

    pub(crate) fn expand_item_from_env(&self, idx: usize) -> TokenStream {
        let field_name = self.field_name(idx);

        if self.skip {
            return quote! {
                let #field_name = Default::default();
            };
        }

        let fn_invoc = if let Some(ref env_var) = self.env_var {
            quote! { FromEnvVar::from_env_var(#env_var) }
        } else {
            quote! { FromEnv::from_env() }
        };

        if self.infallible {
            quote! {
                let #field_name = #fn_invoc
                    .map_err(FromEnvErr::infallible_into)?;
            }
        } else {
            quote! {
                let #field_name = #fn_invoc?;
            }
        }
    }
}
