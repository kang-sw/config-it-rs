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
//! - `env = "<literal>"`: Read default value from environment variable.
//! - `env_once = "<literal>"`: Same as env, but caches value only once.
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
use proc_macro_error::{emit_error, emit_warning, proc_macro_error};
use quote::{quote, quote_spanned};
use syn::{
    punctuated::Punctuated, spanned::Spanned, Attribute, Expr, ExprLit, Lit, LitStr, Meta, Token,
};

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
    let n_props = fn_props.len();

    quote!(
        #[allow(unused_parens)]
        impl #this_crate::Template for #ident {
            fn props__() -> &'static [#this_crate::beacon::group::Property] {
                static PROPS: std::sync::OnceLock<[#this_crate::beacon::group::Property; #n_props]> = std::sync::OnceLock::new();
                PROPS.get_or_init(|| [#(#fn_props)*] )
            }

            fn prop_at_offset__(offset: usize) -> Option<&'static #this_crate::beacon::group::Property> {
                let index = match offset { #(#fn_prop_at_offset)* _ => None::<usize> };
                index.map(|x| &Self::props__()[x])
            }

            fn template_name() -> (&'static str, &'static str) {
                (module_path!(), stringify!(#ident))
            }

            fn default_config() -> Self {
                Self {
                    #(#fn_default_config)*
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

    for (field_index, field) in fields.into_iter().enumerate() {
        let field_span = field.span();
        let field_ty = field.ty;
        let field_ident = field.ident.expect("This is struct with named fields");

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
                let span = meta.span();
                let Meta::NameValue(expr) = meta else {
                    emit_error!(meta, "Expected expression");
                    continue;
                };

                let Some(expr) = expr_take_lit_str(expr.value) else {
                    emit_error!(span, "Expected string literal");
                    continue;
                };

                let span = expr.span();

                let Ok(expr) = syn::parse_str::<Expr>(&expr.value()) else {
                    emit_error!(span, "Expected valid expression");
                    continue;
                };

                field_type = FieldType::PlainWithDefaultExpr(span, expr);
            }
        }

        /* -------------------------------------------------------------------------------------- */
        /*                                   FUNCTION GENERATION                                  */
        /* -------------------------------------------------------------------------------------- */
        let prop = match field_type {
            FieldType::Plain => continue,
            FieldType::PlainWithDefaultExpr(span, expr) => {
                fn_default_config.push(quote_spanned!(span => #field_ident: #expr,));
                continue;
            }
            FieldType::Property(x) => x,
        };

        /* --------------------------------- Metadata Generation -------------------------------- */
        // TODO

        /* --------------------------------- Default Generation --------------------------------- */
        let default_expr = match prop.default {
            Some(FieldPropertyDefault::Expr(expr)) => {
                quote_spanned!(expr.span() => (#expr).to_owned())
            }

            Some(FieldPropertyDefault::ExprStr(lit)) => {
                let span = lit.span();
                let Ok(expr) = syn::parse_str::<Expr>(&lit.value()) else {
                    emit_error!(span, "Expected valid expression");
                    continue;
                };

                quote_spanned!(span => #expr)
            }

            None => {
                quote_spanned!(field_span => Default::default())
            }
        };

        let default_expr = if let Some((once, env)) = prop.env {
            let env_var = env.value();
            if once {
                quote_spanned!(env.span() => {
                    static ENV: ::std::sync::OnceLock<Option<#field_ty>> = ::std::sync::OnceLock::new();
                    ENV .get_or_init(|| std::env::var(#env_var).ok().and_then(|x| x.parse().ok()))
                        .clone().unwrap_or_else(|| #default_expr)
                })
            } else {
                quote_spanned!(env.span() =>
                    std::env::var(#env_var).ok().and_then(|x| x.parse().ok()).unwrap_or_else(|| #default_expr)
                )
            }
        } else {
            default_expr
        };

        fn_default_config.push(quote_spanned!(field_span => #field_ident: #default_expr,));

        /* ------------------------------ Index Access Genenration ------------------------------ */
        // TODO
    }
}

fn from_meta_list(meta_list: syn::MetaList) -> Option<FieldProperty> {
    let mut r = FieldProperty::default();
    let span = meta_list.span();
    let Ok(parsed) =
        meta_list.parse_args_with(<Punctuated<syn::Meta, Token![,]>>::parse_terminated)
    else {
        emit_error!(span, "Expected valid list of arguments");
        return None;
    };

    for arg in parsed {
        let is_ = |x: &str| arg.path().is_ident(x);
        match arg {
            Meta::Path(_) => {
                if is_("admin") {
                    r.admin = true
                } else if is_("admin_write") {
                    r.admin_write = true
                } else if is_("admin_read") {
                    r.admin_read = true
                } else if is_("transient") {
                    r.transient = true
                } else if is_("no_export") {
                    r.no_export = true
                } else if is_("no_import") {
                    r.no_import = true
                } else if is_("hide") {
                    r.hide = true
                } else {
                    emit_warning!(arg, "Unknown attribute")
                }
            }
            Meta::List(_) => {
                emit_warning!(arg, "Unexpected list")
            }
            Meta::NameValue(syn::MetaNameValue { value, path, .. }) => {
                let is_ = |x: &str| path.is_ident(x);
                if is_("default") {
                    r.default = Some(FieldPropertyDefault::Expr(value));
                } else if is_("default_expr") {
                    r.default = expr_take_lit_str(value).map(FieldPropertyDefault::ExprStr);
                } else if is_("alias") {
                    r.alias = expr_take_lit_str(value);
                } else if is_("min") {
                    r.min = Some(value);
                } else if is_("max") {
                    r.max = Some(value);
                } else if is_("one_of") {
                    let Expr::Array(one_of) = value else {
                        emit_error!(value, "Expected array literal");
                        continue;
                    };

                    r.one_of = Some(one_of);
                } else if is_("env_once") {
                    r.env = expr_take_lit_str(value).map(|x| (true, x));
                } else if is_("env") {
                    r.env = expr_take_lit_str(value).map(|x| (false, x));
                } else if is_("editor") {
                    r.editor = Some(value);
                }
            }
        }
    }

    Some(r)
}

enum FieldType {
    Plain,
    PlainWithDefaultExpr(Span, Expr),
    Property(FieldProperty),
}

fn expr_take_lit_str(expr: Expr) -> Option<LitStr> {
    if let Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) = expr {
        Some(lit)
    } else {
        emit_error!(expr, "Expected string literal");
        None
    }
}

enum FieldPropertyDefault {
    Expr(Expr),
    ExprStr(LitStr),
}

#[derive(Default)]
struct FieldProperty {
    alias: Option<syn::LitStr>,
    default: Option<FieldPropertyDefault>,
    admin: bool,
    admin_write: bool,
    admin_read: bool,
    min: Option<syn::Expr>,
    max: Option<syn::Expr>,
    one_of: Option<syn::ExprArray>,
    env: Option<(bool, syn::LitStr)>, // (IsOnce, EnvKey)
    transient: bool,
    no_export: bool,
    no_import: bool,
    editor: Option<syn::Expr>,
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
