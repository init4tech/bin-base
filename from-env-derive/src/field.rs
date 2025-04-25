use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, LitStr, spanned::Spanned};

/// A parsed Field of a struct
pub(crate) struct Field {
    env_var: Option<LitStr>,
    field_name: Option<Ident>,
    field_type: syn::Type,

    optional: bool,
    infallible: bool,
    desc: Option<String>,

    _attrs: Vec<syn::Attribute>,

    span: proc_macro2::Span,
}

impl From<&syn::Field> for Field {
    fn from(field: &syn::Field) -> Self {
        let mut optional = false;
        let mut env_var = None;
        let mut infallible = false;
        let mut desc = None;

        field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("from_env"))
            .for_each(|attr| {
                let _ = attr.parse_nested_meta(|meta| {
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

        // check if the from_env_attrs_field contains either
        // - `from_env(var = "FIELD_NAME")` or
        // - `from_env(optional)`

        let field_type = field.ty.clone();
        let field_name = field.ident.clone();
        let span = field.span();

        Field {
            env_var,
            field_name,
            field_type,
            optional,
            infallible,
            desc,
            _attrs: field
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("from_env"))
                .cloned()
                .collect(),
            span,
        }
    }
}

impl Field {
    pub(crate) fn trait_name(&self) -> TokenStream {
        self.env_var
            .as_ref()
            .map(|_| quote! { FromEnvVar })
            .unwrap_or(quote! { FromEnv })
    }

    pub(crate) fn as_trait(&self) -> TokenStream {
        let field_trait = self.trait_name();
        let field_type = &self.field_type;

        quote! { <#field_type as #field_trait> }
    }

    pub(crate) fn assoc_err(&self) -> TokenStream {
        let as_trait = self.as_trait();

        quote! { #as_trait::Error }
    }

    pub(crate) fn field_name(&self, idx: usize) -> Ident {
        if let Some(field_name) = self.field_name.as_ref() {
            return field_name.clone();
        }

        let n = format!("field_{}", idx);
        syn::parse_str::<Ident>(&n)
            .map_err(|_| syn::Error::new(self.span, "Failed to create field name"))
            .unwrap()
    }

    /// Produces the name of the enum variant for the field
    pub(crate) fn enum_variant_name(&self, idx: usize) -> TokenStream {
        let n = self.field_name(idx).to_string().to_pascal_case();

        let n: Ident = syn::parse_str::<Ident>(&n)
            .map_err(|_| syn::Error::new(self.span, "Failed to create field name"))
            .unwrap();

        quote! { #n }
    }

    /// Produces the variant, containing the error type
    pub(crate) fn expand_enum_variant(&self, idx: usize) -> TokenStream {
        let variant_name = self.enum_variant_name(idx);
        let var_name_str = variant_name.to_string();
        let assoc_err = self.assoc_err();

        if self.infallible {
            return quote! {
                #[doc = "Error for "]
                #[doc = #var_name_str]
                #variant_name
            };
        }

        quote! {
            #[doc = "Error for "]
            #[doc = #var_name_str]
            #variant_name(#assoc_err)
        }
    }

    /// Produces the a line for the `inventory` function
    /// of the form
    /// items.push(...);
    /// or
    /// items.extend(...);
    pub(crate) fn expand_env_item_info(&self) -> TokenStream {
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

    pub(crate) fn expand_variant_display(&self, idx: usize) -> TokenStream {
        let variant_name = self.enum_variant_name(idx);

        if self.infallible {
            return quote! {
                Self::#variant_name => unreachable!("Infallible")
            };
        }

        quote! {
            Self::#variant_name(err) => err.fmt(f)
        }
    }

    pub(crate) fn expand_variant_source(&self, idx: usize) -> TokenStream {
        let variant_name = self.enum_variant_name(idx);

        if self.infallible {
            return quote! {
                Self::#variant_name => unreachable!("Infallible")
            };
        }

        quote! {
            Self::#variant_name(err) => Some(err)
        }
    }

    pub(crate) fn expand_item_from_env(&self, err_ident: &Ident, idx: usize) -> TokenStream {
        // Produces code fo the following form:
        // ```rust
        // // EITHER
        // let field_name = env::var(#self.env_var.unwrap()).map_err(|e| e.map(#ErroEnum::FieldName))?;

        // // OR
        // let field_name =  FromEnvVar::from_env_var(#self.env_var.unwrap()).map_err(|e| e.map(#ErroEnum::FieldName))?;

        // // OR
        // let field_name =  FromEnv::from_env().map_err()?;
        //```
        let variant = self.enum_variant_name(idx);
        let field_name = self.field_name(idx);

        let fn_invoc = if let Some(ref env_var) = self.env_var {
            quote! { FromEnvVar::from_env_var(#env_var) }
        } else {
            quote! { FromEnv::from_env() }
        };

        let map_line = if self.infallible {
            quote! { FromEnvErr::infallible_into }
        } else {
            quote! { |e| e.map(#err_ident::#variant) }
        };

        quote! {
            let #field_name = #fn_invoc
                .map_err(#map_line)?;
        }
    }
}
