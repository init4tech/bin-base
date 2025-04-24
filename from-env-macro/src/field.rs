use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, LitStr, spanned::Spanned};

/// A parsed Field of a struct
pub(crate) struct Field {
    env_var: Option<LitStr>,
    field_name: Option<Ident>,
    field_type: syn::Type,

    span: proc_macro2::Span,
}

impl From<&syn::Field> for Field {
    fn from(field: &syn::Field) -> Self {
        let env_var = field
            .attrs
            .iter()
            .filter_map(|attr| attr.meta.require_list().ok())
            .find(|attr| attr.path.is_ident("from_env_var"))
            .and_then(|attr| attr.parse_args::<LitStr>().ok());

        let field_type = field.ty.clone();
        let field_name = field.ident.clone();
        let span = field.span();

        Field {
            env_var,
            field_name,
            field_type,
            span,
        }
    }
}

impl Field {
    pub(crate) fn enum_variant_name(&self, idx: usize) -> TokenStream {
        eprintln!("Field name: {:?}", self.field_name);
        let n = if let Some(field_name) = self.field_name.as_ref() {
            field_name.to_string()
        } else {
            format!("Field{}", idx)
        }
        .to_pascal_case();

        syn::parse_str::<Ident>(&n)
            .map_err(|_| syn::Error::new(self.span, "Failed to create field name"))
            .unwrap();

        eprintln!("Field name: {}", n);

        return quote! { #n };
    }

    pub(crate) fn expand_enum_variant(&self, idx: usize) -> TokenStream {
        let field_name = self.enum_variant_name(idx);
        let field_type = &self.field_type;
        let field_trait = if self.env_var.is_some() {
            quote! { FromEnv }
        } else {
            quote! { FromEnvErr }
        };
        quote! {
            #[doc = "Error for" #field_name]
            #field_name(<#field_type as #field_trait>::Error)
        }
    }
}
