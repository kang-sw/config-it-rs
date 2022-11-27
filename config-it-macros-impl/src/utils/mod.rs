#![allow(dead_code)]
pub mod parsing;

use parsing::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use std::mem::replace;

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
            .map_or_else(|| ident_str.clone(), |x| x.into_token_stream().to_string());
        let doc = x.docstring;

        let default = x.default_value.map_or(quote!{Default::default()}, |x| x.into_token_stream());
        let min = x.min.clone().map_or(quote!{None}, |x| quote!{Some(#x)} );
        let max = x.max.clone().map_or(quote!{None}, |x| quote!(Some(#x)) );
        let one_of = x.one_of.clone().map_or(quote!(), |x| {
            let args = x.nested.into_iter().map(|x| quote!{#x.into(), });
            quote!{#(#args)*}
        });

        let disable_import = x.flag_disable_import;
        let disable_export = x.flag_disable_export;
        
        let hidden = x.flag_hidden;
        
        let func_min = x.min.map_or(quote!{}, |x| {
            quote! {
                if *to < #x {
                    *to = (#x).into();
                    result = Some(false);
                }
            }
        });
        
        let func_max = x.max.map_or(quote!{}, |x| {
            quote! {
                if *to > #x {
                    *to = (#x).into();
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
                    v_default: #default,
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

        indexer += 1;
        
        let elem_at = quote!{
            #indexer => &mut self.#ident,
        };
        
        (meta_gen, elem_at)
    });

    let mut vec_fields = Vec::with_capacity(num_fields);
    let mut vec_idents = Vec::with_capacity(num_fields);

    fields.for_each(|(a, b)| {
        vec_fields.push(a);
        vec_idents.push(b);
    });

    // TODO: Implement Default for generated struct
    
    Ok(quote! {
        #[allow(unused)]
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
        }
    })
}
