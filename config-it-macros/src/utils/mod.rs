#![allow(dead_code)]
pub mod parsing;

use parsing::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use std::mem::replace;
use std::str::FromStr;
use syn::Lit;

pub fn generate(mut ty: TypeDesc) -> Result<TokenStream, (Span, String)> {
    let identifier = ty.identifier;
    let generics = ty.generics;

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

        let default_to_meta = default.as_ref().map_or(quote!(Default::default()), |v| quote!(#v));
        let min = x.min.as_ref().map_or(quote!{None}, |x| quote!{Some(#x)} );
        let max = x.max.as_ref().map_or(quote!{None}, |x| quote!(Some(#x)) );
        let one_of = x.one_of.as_ref().map_or(quote!(), |x| {
            let args = x.nested.iter().map(|x| quote!{#x.into(), });
            quote!{#(#args)*}
        });

        let disable_import = x.flag_disable_import;
        let disable_export = x.flag_disable_export;

        let hidden = x.flag_hidden;

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

                let init = config_it::entity::MetadataValInit::<Type> {
                    fn_validate: |_meta, to| -> Option<bool> {

                        let to: &mut Type = to.downcast_mut().unwrap();
                        let mut result = Some(true);

                        #func_min
                        #func_max
                        #func_one_of

                        result
                    },
                    v_default: #default_to_meta,
                    v_one_of: [#one_of].into(),
                    v_max: #min,
                    v_min: #max,
                };

                let props = config_it::entity::MetadataProps {
                    description: doc_string,
                    varname,
                    disable_import: #disable_import,
                    disable_export: #disable_export,
                    hidden: #hidden,
                };

                let meta = config_it::entity::Metadata::create_for_base_type(identifier, init, props);

                let prop_data = config_it::config::PropData {
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
                        }
                    }

                    if !env_parsed {
                        #default_val
                    }
                }
            }
        );

        indexer += 1;
        (meta_gen, elem_at, default_val)
    });

    let mut vec_fields = Vec::with_capacity(num_fields);
    let mut vec_idents = Vec::with_capacity(num_fields);
    let mut vec_defaults = Vec::with_capacity(num_fields);

    fields.for_each(|(a, b, c)| {
        vec_fields.push(a);
        vec_idents.push(b);
        vec_defaults.push(c);
    });

    Ok(quote! {
        #[allow(dead_code)]
        impl #generics config_it::ConfigGroupData for #identifier #generics {
            fn prop_desc_table__() -> &'static std::collections::HashMap<usize, config_it::config::PropData> {
                use config_it::lazy_static;

                lazy_static! {
                    static ref TABLE: std::sync::Arc<std::collections::HashMap<usize, config_it::config::PropData>> = {
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
        }
    })
}
