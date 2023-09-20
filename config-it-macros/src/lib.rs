//!
//!
//! # Usage
//!
//! # Attributes
//!
//! All attributes are put inside of `#[config(...)]` or `#[config_it(...)]` attribute.
//!
//! - `alias = "<name>"`: Set alias for the field.
//! - `default = <expr>`: Set default value for the field.
//! - `default_expr = "<expr>"`: Set default value for the field.
//! - `admin | admin_write | admin_read`: Prohibit access to the field for the user.
//! - `min = <expr> | max = <expr> | one_of = [<expr>...]`: Sets constraint for the field
//! - `env = "<literal>"`: Sets environment variable name for the field.
//! - `transient | no_export | no_import`: Prohibit export/import of the field.
//! - `editor = $this::MetadataEditorHint::<ident>`: Sets editor hint for the field.
//! - `hide`: Hide field from the editor.
//!
//! # Using with non-config-it types
//!
//! For types which are not part of configuration, but does not provides `Default` trait, you can
//! use `#[non_config_default_expr = "<expr>"]` attribute to provide default for these types.
//!

use proc_macro::TokenStream as LangTokenStream;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::{emit_error, proc_macro_error};
use quote::{quote, quote_spanned};
use syn::{punctuated::Punctuated, spanned::Spanned, Attribute, Expr, Meta, Token};

#[proc_macro_error]
#[proc_macro_derive(Template, attributes(config_it, config, non_config_default_expr))]
pub fn derive_collect_fn(item: LangTokenStream) -> LangTokenStream {
    let tokens = TokenStream::from(item);
    let Ok(syn::ItemStruct { attrs: _, ident, fields, .. }) =
        syn::parse2::<syn::ItemStruct>(tokens)
    else {
        proc_macro_error::abort_call_site!("expected struct")
    };
    let syn::Fields::Named(fields) = fields else {
        proc_macro_error::abort_call_site!("Non-named fields are not allowed")
    };

    let mut gen = GenContext::default();
    let this_crate = this_crate_name();

    visit_fields(
        &mut gen,
        GenInputCommon { this_crate: &this_crate, struct_ident: &ident },
        fields,
    );

    let GenContext {
        // <br>
        fn_props,
        fn_prop_at_offset,
        fn_default_config,
        fn_elem_at_mut,
        ..
    } = gen;

    quote!(
        impl #this_crate::Template for #ident {
            fn props__() -> &'static [# this_crate::config::PropDesc] {
                #this_crate::lazy_static! {
                    static ref PROPS: &'static [#this_crate::config::PropDesc] = &[#(#fn_props),*];
                };

                *PROPS
            }

            fn prop_at_offset__(offset: usize) -> Option<&'static #this_crate::config::PropDesc> {
                match offset { #(#fn_prop_at_offset)* _ => None::<usize> }.map(|x| &Self::props__()[x])
            }

            fn template_name() -> (&'static str, &'static str) {
                (module_path!(), stringify!(#ident))
            }

            fn default_config() -> Self {
                Self {
                    #(#fn_default_config),*
                    ..todo!()
                }
            }

            fn elem_at_mut__(&mut self, index: usize) -> &mut dyn std::any::Any {
                match index {
                    #(#fn_elem_at_mut)*
                    _ => panic!("Invalid index {}", index),
                }
            }
        }
    )
    .into()
}

fn visit_fields(
    GenContext { fn_props, fn_prop_at_offset, fn_default_config, fn_elem_at_mut }: &mut GenContext,
    GenInputCommon { this_crate, struct_ident }: GenInputCommon,
    syn::FieldsNamed { named: fields, .. }: syn::FieldsNamed,
) {
    let n_field = fields.len();
    fn_prop_at_offset.reserve(n_field);
    fn_default_config.reserve(n_field);
    fn_props.reserve(n_field);
    fn_elem_at_mut.reserve(n_field);

    let mut doc_string = Vec::new();

    for field in fields.into_iter() {
        /* -------------------------------------------------------------------------------------- */
        /*                                    ATTRIBUTE PARSING                                   */
        /* -------------------------------------------------------------------------------------- */

        let mut field_type = FieldType::Plain;
        doc_string.clear();

        for Attribute { meta, .. } in field.attrs {
            if meta.path().is_ident("doc") {
                /* ----------------------------- Doc String Parsing ----------------------------- */
                let Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit_str), .. }) =
                    &meta.require_name_value().unwrap().value
                else {
                    proc_macro_error::abort!(meta, "Expected string literal")
                };

                doc_string.push(lit_str.value());
            } else if ["config", "config_it"].into_iter().any(|x| meta.path().is_ident(x)) {
                /* ------------------------------ Config Attributes ----------------------------- */
                if !matches!(&field_type, FieldType::Plain) {
                    emit_error!(meta, "Duplicate config attribute");
                    continue;
                }

                field_type = FieldType::Property(match meta {
                    Meta::Path(_) => Default::default(),
                    Meta::List(list) => {
                        if let Some(x) = from_meta_list(list) {
                            x
                        } else {
                            continue;
                        }
                    }
                    Meta::NameValue(_) => {
                        emit_error!(meta, "Unexpected value. Expected `#[config(...)]`");
                        continue;
                    }
                });
            } else if meta.path().is_ident("non_config_default_expr") {
                /* --------------------------------- Non-config --------------------------------- */
                let Meta::NameValue(expr) = meta else {
                    emit_error!(meta, "Expected expression");
                    continue;
                };

                field_type = FieldType::PlainWithDefaultExpr { expr: expr.value };
            }
        }

        /* -------------------------------------------------------------------------------------- */
        /*                                   FUNCTION GENERATION                                  */
        /* -------------------------------------------------------------------------------------- */
    }
}

fn from_meta_list(meta_list: syn::MetaList) -> Option<FieldTypeProperty> {
    let mut r = FieldTypeProperty::default();
    let span = meta_list.span();
    let Ok(parsed) =
        meta_list.parse_args_with(<Punctuated<syn::Meta, Token![,]>>::parse_terminated)
    else {
        emit_error!(span, "Expected valid list of arguments");
        return None;
    };

    for args in parsed {}
    Some(r)
}

enum FieldType {
    Plain,
    PlainWithDefaultExpr { expr: syn::Expr },
    Property(FieldTypeProperty),
}

#[derive(Default)]
struct FieldTypeProperty {
    alias: Option<syn::LitStr>,
    default: Option<syn::Expr>,
    default_expr: Option<syn::LitStr>,
    admin: bool,
    admin_write: bool,
    admin_read: bool,
    min: Option<syn::Expr>,
    max: Option<syn::Expr>,
    one_of: Option<Vec<syn::Expr>>,
    env: Option<syn::LitStr>,
    transient: bool,
    no_export: bool,
    no_import: bool,
    editor: Option<syn::Meta>,
    hide: bool,
}

fn this_crate_name() -> TokenStream {
    use proc_macro_crate::*;

    match crate_name("config-it") {
        Ok(FoundCrate::Itself) => quote!(::config_it),
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }

        Err(_) => {
            // HACK: We may handle the re-exported crate that was aliased as 'config_it'
            quote!(config_it)
        }
    }
}

struct GenInputCommon<'a> {
    this_crate: &'a TokenStream,
    struct_ident: &'a syn::Ident,
}

#[derive(Default)]
struct GenContext {
    fn_props: Vec<TokenStream>,
    fn_prop_at_offset: Vec<TokenStream>,
    fn_default_config: Vec<TokenStream>,
    fn_elem_at_mut: Vec<TokenStream>,
}
