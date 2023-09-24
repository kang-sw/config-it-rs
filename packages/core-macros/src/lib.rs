use std::borrow::Cow;

use proc_macro::TokenStream as LangTokenStream;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::{emit_error, proc_macro_error};
use quote::{quote, quote_spanned};
use syn::{
    punctuated::Punctuated, spanned::Spanned, Attribute, Expr, ExprLit, Ident, Lit, LitStr, Meta,
    Token,
};

/// # Usage
///
/// ```ignore
/// #[derive(config_it::Template, Clone)]
/// struct MyConfig {
///     /// Documentation comments are retrieved and used as descriptions in the editor.
///     #[config(default = 154)]
///     pub any_number: i32,
///     
///     #[non_config_default_expr = r#"1.try_into().unwrap()"#]
///     pub non_config_number: std::num::NonZeroUsize,
///     
///     pub non_config_boolean: bool,
/// }
///
/// let storage = config_it::create_storage();
/// let mut config = storage.find_or_create::<MyConfig>(["my", "config", "path"]).unwrap();
///
/// // Initial update always reports dirty.
/// assert_eq!(config.update(), true);
/// assert_eq!(config.consume_update(&config.any_number), true);
/// assert_eq!(config.consume_update(&config.non_config_number), true);
/// assert_eq!(config.consume_update(&config.non_config_boolean), true);
/// ```
///
/// # Attributes
///
/// Attributes are encapsulated within `#[config(...)]` or `#[config_it(...)]`.
///
/// - `alias = "<name>"`: Assign an alias to the field.
/// - `default = <expr>` or `default_expr = "<expr>"`: Define a default value for the field.
/// - `admin | admin_write | admin_read`: Restrict access to the field for non-admin users.
///
/// - `min = <expr>`, `max = <expr>`, `one_of = [<expr>...]`: Apply constraints to the field.
/// - `validate_with = "<function_name>"`: Specify a validation function for the field with the
///   signature `fn(&mut T) -> Result<Validation, impl Into<Cow<'static, str>>>`.
///
/// - `readonly | writeonly`: Designate an element as read-only or write-only.
/// - `secret`: Flag an element as confidential (e.g., passwords). The value will be archived as a
///   encrypted base64 string, which is becomes readable when imported back by storage.
///
/// - `env = "<literal>"` or `env_once = "<literal>"`: Set the default value from an environment
///   variable.
/// - `transient | no_export | no_import`: Prevent field export/import.
/// - `editor = <ident>`: Define an editor hint for the field. See
///   [`config_it::shared::meta::MetadataEditorHint`]
///
/// - `hidden` or `hidden_non_admin`: Make a field invisible in the editor or only to non-admin
///   users, respectively.
///
/// # Interacting with non-config-it Types
///
/// For non-configuration types that lack a `Default` trait, the `#[non_config_default_expr =
/// "<expr>"]` attribute can be used to specify default values.

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
        fn_global_constants,
        ..
    } = gen;
    let n_props = fn_props.len();

    quote!(
        #[allow(unused_parens)]
        #[allow(unused_braces)]
        #[allow(clippy::useless_conversion)]
        #[allow(clippy::redundant_closure)]
        const _: () = {
            #( #fn_global_constants )*

            impl #this_crate::Template for #ident {
                type LocalPropContextArray = #this_crate::config::group::LocalPropContextArrayImpl<#n_props>;

                fn props__() -> &'static [#this_crate::config::entity::PropertyInfo] {
                    static PROPS: ::std::sync::OnceLock<[#this_crate::config::entity::PropertyInfo; #n_props]> = ::std::sync::OnceLock::new();
                    PROPS.get_or_init(|| [#(#fn_props)*] )
                }

                fn prop_at_offset__(offset: usize) -> Option<&'static #this_crate::config::entity::PropertyInfo> {
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
                    use ::std::any::Any;

                    match index {
                        #(#fn_elem_at_mut)*
                        _ => panic!("Invalid index {}", index),
                    }
                }
            }
        };
    )
    .into()
}

fn visit_fields(
    GenContext {
        fn_props,
        fn_prop_at_offset,
        fn_default_config,
        fn_elem_at_mut,
        fn_global_constants,
    }: &mut GenContext,
    GenInputCommon { this_crate, struct_ident }: GenInputCommon,
    syn::FieldsNamed { named: fields, .. }: syn::FieldsNamed,
) {
    let n_field = fields.len();
    fn_prop_at_offset.reserve(n_field);
    fn_default_config.reserve(n_field);
    fn_props.reserve(n_field);
    fn_elem_at_mut.reserve(n_field);

    let mut doc_string = Vec::new();
    let mut field_index = 0usize;

    for field in fields.into_iter() {
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

                let Ok(expr) = expr.parse::<Expr>() else {
                    emit_error!(span, "Expected valid expression");
                    continue;
                };

                field_type = FieldType::PlainWithDefaultExpr(span, expr);
            } else {
                // Safely ignore unknown attributes
            }
        }

        /* -------------------------------------------------------------------------------------- */
        /*                                   FUNCTION GENERATION                                  */
        /* -------------------------------------------------------------------------------------- */
        let prop = match field_type {
            FieldType::Plain => {
                fn_default_config
                    .push(quote_spanned!(field_span => #field_ident: Default::default(),));
                continue;
            }
            FieldType::PlainWithDefaultExpr(span, expr) => {
                fn_default_config.push(quote_spanned!(span => #field_ident: #expr,));
                continue;
            }
            FieldType::Property(x) => x,
        };

        /* --------------------------------- Default Generation --------------------------------- */
        let default_expr = match prop.default {
            Some(FieldPropertyDefault::Expr(expr)) => {
                quote_spanned!(expr.span() => <#field_ty>::try_from(#expr).unwrap())
            }

            Some(FieldPropertyDefault::ExprStr(lit)) => {
                let Ok(expr) = lit.parse::<Expr>() else {
                    emit_error!(lit.span(), "Expected valid expression");
                    continue;
                };

                quote!(#expr)
            }

            None => {
                quote_spanned!(field_span => Default::default())
            }
        };

        let default_expr = if let Some((once, env)) = prop.env.clone() {
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

        let default_fn_ident = format!("__fn_default_{}", field_ident);
        let default_fn_ident = Ident::new(&default_fn_ident, field_ident.span());
        let field_ident_upper = field_ident.to_string().to_uppercase();
        let const_offset_ident =
            Ident::new(&format!("__COFST_{field_ident_upper}"), Span::call_site());

        fn_global_constants.push(quote_spanned!(field_span =>
            fn #default_fn_ident() -> #field_ty {
                #default_expr
            }

            const #const_offset_ident: usize = #this_crate::offset_of!(#struct_ident, #field_ident);
        ));

        fn_default_config.push(quote_spanned!(field_span => #field_ident: #default_fn_ident(),));

        /* --------------------------------- Metadata Generation -------------------------------- */
        {
            let FieldProperty {
                rename,
                admin,
                admin_write,
                admin_read,
                min,
                max,
                one_of,
                transient,
                no_export,
                no_import,
                editor,
                hidden,
                secret,
                readonly,
                writeonly,
                env,
                validate_with,
                ..
            } = prop;

            let flags = [
                readonly.then(|| quote!(MetaFlag::READONLY)),
                writeonly.then(|| quote!(MetaFlag::WRITEONLY)),
                hidden.then(|| quote!(MetaFlag::HIDDEN)),
                secret.then(|| quote!(MetaFlag::SECRET)),
                admin.then(|| quote!(MetaFlag::ADMIN)),
                admin_write.then(|| quote!(MetaFlag::ADMIN_WRITE)),
                admin_read.then(|| quote!(MetaFlag::ADMIN_READ)),
                transient.then(|| quote!(MetaFlag::TRANSIENT)),
                no_export.then(|| quote!(MetaFlag::NO_EXPORT)),
                no_import.then(|| quote!(MetaFlag::NO_IMPORT)),
            ]
            .into_iter()
            .flatten();

            let varname = field_ident.to_string();
            let name = rename
                .map(|x| Cow::Owned(x.value()))
                .unwrap_or_else(|| Cow::Borrowed(varname.as_str()));
            let doc_string = doc_string.join("\n");
            let none = quote!(None);
            let env = env
                .as_ref()
                .map(|x| x.1.value())
                .map(|env| quote!(Some(#env)))
                .unwrap_or_else(|| none.clone());
            let editor_hint = editor
                .map(|x| {
                    quote!(Some(
                        __meta::MetadataEditorHint::#x
                    ))
                })
                .unwrap_or_else(|| none.clone());

            let schema = cfg!(feature = "jsonschema").then(|| {
                quote! {
                    schema: {
                        __default_ref_ptr::<#field_ty>().get_schema()
                    }
                }
            });
            let validation_function = {
                let fn_min = min.map(|x| {
                    quote_spanned!(x.span() => {
                        if *mref < #x {
                            editted = true;
                            *mref = #x;
                        }
                    })
                });
                let fn_max = max.map(|x| {
                    quote_spanned!(x.span() => {
                        if *mref > #x {
                            editted = true;
                            *mref = #x;
                        }
                    })
                });
                let fn_one_of = one_of.map(|x| {
                    quote_spanned!(x.span() => {
                        if #x.into_iter().all(|x| x != *mref) {
                            return Err("Value is not one of the allowed values".into());
                        }
                    })
                });
                let fn_user = validate_with.map(|x| {
                    let Ok(ident) = x.parse::<syn::ExprPath>() else {
                        emit_error!(x, "Expected valid identifier");
                        return none.clone();
                    };
                    quote_spanned!(x.span() => {
                        match #ident(mref) {
                            Ok(__entity::Validation::Valid) => {}
                            Ok(__entity::Validation::Modified) => { editted = true }
                            Err(e) => return Err(e),
                        }
                    })
                });

                quote! {
                    let mut editted = false;

                    #fn_min
                    #fn_max
                    #fn_one_of
                    #fn_user

                    if !editted {
                        Ok(__entity::Validation::Valid)
                    } else {
                        Ok(__entity::Validation::Modified)
                    }
                }
            };

            fn_props.push(quote_spanned! { field_span =>
                {
                    use #this_crate::config as __config;
                    use #this_crate::shared as __shared;
                    use __config::entity as __entity;
                    use __config::__lookup::*;
                    use __shared::meta as __meta;

                    __entity::PropertyInfo::new(
                        /* type_id:*/ std::any::TypeId::of::<#field_ty>(),
                        /* index:*/ #field_index,
                        /* metadata:*/ __meta::Metadata {
                            flags: {
                                use __meta::MetaFlag;
                                #(#flags |)* MetaFlag::empty()
                            },
                            name: #name,
                            type_name: stringify!(#field_ty),
                            varname: #varname,
                            description: #doc_string,
                            env: #env,
                            editor_hint: #editor_hint,
                            #schema
                        },
                        /* vtable:*/ Box::leak(Box::new(__entity::MetadataVTableImpl {
                            impl_copy: #this_crate::impls!(#field_ty: Copy),
                            fn_default: #default_fn_ident,
                            fn_validate: {
                                fn __validate(mref: &mut #field_ty) -> __entity::ValidationResult {
                                    let _ = mref; // Allow unused instance
                                    #validation_function
                                }

                                __validate
                            },
                        }))
                    )
                },
            });
        }

        /* ------------------------------ Index Access Genenration ------------------------------ */
        fn_prop_at_offset.push(quote!(#const_offset_ident => Some(#field_index),));
        fn_elem_at_mut.push(quote!(#field_index => &mut self.#field_ident as &mut dyn Any,));

        /* -------------------------------- Field Index Increment ------------------------------- */
        field_index += 1;
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
                } else if is_("secret") {
                    r.secret = true
                } else if is_("readonly") {
                    r.readonly = true
                } else if is_("writeonly") {
                    r.writeonly = true
                } else if is_("hidden") {
                    r.hidden = true
                } else {
                    emit_error!(arg, "Unknown attribute")
                }
            }
            Meta::List(_) => {
                emit_error!(arg, "Unexpected list")
            }
            Meta::NameValue(syn::MetaNameValue { value, path, .. }) => {
                let is_ = |x: &str| path.is_ident(x);
                if is_("default") {
                    r.default = Some(FieldPropertyDefault::Expr(value));
                } else if is_("default_expr") {
                    r.default = expr_take_lit_str(value).map(FieldPropertyDefault::ExprStr);
                } else if is_("alias") || is_("rename") {
                    r.rename = expr_take_lit_str(value);
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
                } else if is_("validate_with") {
                    r.validate_with = expr_take_lit_str(value);
                } else if is_("env_once") {
                    r.env = expr_take_lit_str(value).map(|x| (true, x));
                } else if is_("env") {
                    r.env = expr_take_lit_str(value).map(|x| (false, x));
                } else if is_("editor") {
                    r.editor = Some(value);
                } else {
                    emit_error!(path.span(), "Unknown attribute")
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
    rename: Option<syn::LitStr>,
    default: Option<FieldPropertyDefault>,
    admin: bool,
    admin_write: bool,
    admin_read: bool,
    secret: bool,
    readonly: bool,
    writeonly: bool,
    min: Option<syn::Expr>,
    max: Option<syn::Expr>,
    one_of: Option<syn::ExprArray>,
    env: Option<(bool, syn::LitStr)>, // (IsOnce, EnvKey)
    validate_with: Option<syn::LitStr>,
    transient: bool,
    no_export: bool,
    no_import: bool,
    editor: Option<syn::Expr>,
    hidden: bool,
}

fn this_crate_name() -> TokenStream {
    use proc_macro_crate::*;

    match crate_name("config-it") {
        Ok(FoundCrate::Itself) => quote!(::config_it),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
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
    struct_ident: &'a Ident,
}

#[derive(Default)]
struct GenContext {
    fn_props: Vec<TokenStream>,
    fn_prop_at_offset: Vec<TokenStream>,
    fn_global_constants: Vec<TokenStream>,
    fn_default_config: Vec<TokenStream>,
    fn_elem_at_mut: Vec<TokenStream>,
}
