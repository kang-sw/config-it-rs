#![allow(dead_code)]
pub mod parsing;

use parsing::*;
use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use std::mem::replace;
use std::str::FromStr;
use syn::Lit;

pub fn generate(mut ty: TypeDesc) -> Result<TokenStream, (Span, String)> {
    let identifier = ty.identifier;
    let generics = ty.generics;

    // Check if it's aliased during manifest.
    let this_crate = match crate_name("config-it") {
        Ok(FoundCrate::Itself) => quote!(::config_it),
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }

        // HACK: We may handle the re-exported crate that was aliased as 'config_it'
        Err(_) => {
            quote!(config_it)
        }
    };

    let fields = replace(&mut ty.fields, Default::default());
    let num_fields = fields.len();
    let mut indexer: usize = 0;

    let fields = fields.into_iter().map(|x| {
        let ident_str = x.identifier.to_string();
        let ident = &x.identifier;
        let ty = x.src_type;
        let alias = x
            .alias
            .map_or_else(|| ident_str.clone(), |x| match x {
                Lit::Str(x) => x.value(),
                _ => ident_str.clone(),
            });
        let doc = x.docstring;

        let default = if let Some(x) = &x.default_value {
            Some(quote!(#x.try_into().unwrap()))
        } else if let Some(Lit::Str(x)) = &x.default_expr {
            // x is in form of "<expr>". Retrieve <expr> from quotes
            let x = x.value();
            Some(TokenStream::from_str(&x).unwrap())
        } else {
            None
        };

        let default_to_meta = default.as_ref().map_or_else(|| quote!(Default::default()), |v| quote!(#v));
        let min = x.min.as_ref().map_or_else(|| quote!{None}, |x| quote!{Some(#x)} );
        let max = x.max.as_ref().map_or_else(|| quote!{None}, |x| quote!(Some(#x)) );
        let one_of = x.one_of.as_ref().map_or_else(|| quote!(), |x| {
            let args = x.nested.iter().map(|x| quote!{#x.into(), });
            quote!{#(#args)*}
        });

        let disable_import = x.flag_disable_import;
        let disable_export = x.flag_disable_export;

        let access = x.access_level.unwrap_or([AccessLevel::Guest; 2]);
        let [access_r, access_w] = access.map(|x| match x {
            AccessLevel::Off => quote!{#this_crate::AccessLevel::Off},
            AccessLevel::Admin => quote!{#this_crate::AccessLevel::Admin},
            AccessLevel::User => quote!{#this_crate::AccessLevel::User},
            AccessLevel::Guest => quote!{#this_crate::AccessLevel::Guest},
        });

        let func_min = x.min.map_or(quote!{}, |x| {
            quote! {
                if *to < #x {
                    *to = (#x).try_into().unwrap();
                    result = Some(false);
                }
            }
        });

        let func_max = x.max.map_or(quote!{}, |x| {
            quote! {
                if *to > #x {
                    *to = (#x).try_into().unwrap();
                    result = Some(false);
                }
            }
        });

        let func_one_of = if let Some(v) = x.one_of {
            let args =v.nested.into_iter().map(
                |x| quote! {
                    x if *x == #x => true,
                });

            quote! {
                let matches_one_of = match to {
                    #(#args)*
                    _ => false,
                };

                if !matches_one_of {
                    result = None;
                }
            }
        } else {
            quote!()
        };

        let meta_gen = quote! {
            {
                type Type = #ty;

                let offset = unsafe {
                    let owner = 0 as *const #identifier;
                    &(*owner).#ident as *const _ as *const u8 as usize
                };

                let identifier = #alias;
                let varname = #ident_str;
                let doc_string = #doc;
                let index = #indexer as usize;

                #[allow(unused)]
                use #this_crate::entity::lookups::{HasSchema, NoSchema};

                let v_default: Type = #default_to_meta;
                let schema = (&v_default).get_schema();

                let init = #this_crate::entity::MetadataValInit::<Type> {
                    fn_validate: |_meta, to| -> Option<bool> {

                        let to: &mut Type = to.downcast_mut().unwrap();
                        let mut result = Some(true);

                        #func_min
                        #func_max
                        #func_one_of

                        result
                    },
                    v_default,
                    v_one_of: [#one_of].into(),
                    v_max: #min,
                    v_min: #max,
                };

                let props = #this_crate::entity::MetadataProps {
                    description: doc_string,
                    varname,
                    disable_import: #disable_import,
                    disable_export: #disable_export,
                    read_access: #access_r,
                    write_access: #access_w,
                    schema,
                };

                let meta = #this_crate::entity::Metadata::create_for_base_type(identifier, init, props);

                let prop_data = #this_crate::config::PropData {
                    index,
                    type_id: std::any::TypeId::of::<Type>(),
                    meta: std::sync::Arc::new(meta),
                };

                s.insert(offset, prop_data);
            }
        };


        let elem_at = quote!{
            #indexer => &mut self.#ident,
        };

        // If given property has 'env' option, try find corresponding environment variable,
        //  and if one is found, try to parse it. Otherwise, don't touch or use default.
        let default_init = quote! {Default::default()};
        let default_init = default.as_ref().map_or(&default_init, |x| x);
        let default_init = quote! {
            #ident: #default_init,
        };

        let default_val = default.as_ref().map_or(
            quote!{}, |x| quote!{
                self.#ident = #x;
        });

        let env_var = x.env_var;
        let default_val = env_var.as_ref().map_or(
            quote!{#default_val},
            |env_var| {
                quote! {
                    let mut env_parsed = false;
                    if let Ok(x) = std::env::var(#env_var) {

                        // TODO: Custom environment parser
                        // To support types which does not implement 'parse', implement way to
                        //  provide customized environnement parser function, which accepts
                        //  `&str` then returns `Option<U>`, which implements trait `Into<T>()`.
                        if let Ok(x) = x.parse::<#ty>() {
                            self.#ident = x;
                            env_parsed = true;
                        } else {
                            ()
                        }
                    }

                    if !env_parsed {
                        #default_val
                    }
                }
            }
        );


        indexer += 1;
        (meta_gen, elem_at, default_val, default_init)
    });

    let vec_invis = ty.invisibles.iter().map(|x| {
        let ident = &x.identifier;
        let default = match &x.default_expr {
            Some(Lit::Str(x)) => {
                let x = x.value();
                TokenStream::from_str(&x).unwrap()
            }
            Some(x) => {
                quote! {#x.try_into().expect("config_it: Invalid default value specified")}
            }
            None => match &x.default_tokens {
                Some(x) => x.clone(),
                None => quote! {Default::default()},
            },
        };

        quote! {
            #ident: #default,
        }
    });

    let mut vec_fields = Vec::with_capacity(num_fields);
    let mut vec_idents = Vec::with_capacity(num_fields);
    let mut vec_defaults = Vec::with_capacity(num_fields);
    let mut vec_inits = Vec::with_capacity(num_fields);

    fields.for_each(|(a, b, c, d)| {
        vec_fields.push(a);
        vec_idents.push(b);
        vec_defaults.push(c);
        vec_inits.push(d);
    });

    Ok(quote! {
        #[allow(dead_code)]
        impl #generics #this_crate::Template for #identifier #generics {
            fn prop_desc_table__() -> &'static std::collections::HashMap<usize, #this_crate::config::PropData> {
                use #this_crate::lazy_static;

                lazy_static! {
                    static ref TABLE: std::sync::Arc<std::collections::HashMap<usize, #this_crate::config::PropData>> = {
                        let mut s = std::collections::HashMap::new();

                        #(#vec_fields)*

                        std::sync::Arc::new(s)
                    };
                }

                &*TABLE
            }

            fn elem_at_mut__(&mut self, index: usize) -> &mut dyn std::any::Any {
                match index {
                    #(#vec_idents)*
                    _ => unreachable!(),
                }
            }

            fn fill_default(&mut self) {
                #(#vec_defaults)*
            }

            fn default_config() -> Self {
                Self {
                    #(#vec_inits)*
                    #(#vec_invis)*
                }
            }

            fn template_name() -> (&'static str, &'static str) {
                (module_path!(), stringify!(#identifier))
            }
        }
    })
}
