pub mod parsing;

use parsing::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use std::{borrow::Borrow, mem::replace};
use syn::parse2;

pub fn generate(mut ty: TypeDesc) -> Result<TokenStream, (Span, String)> {
    let identifier = ty.identifier;
    let vis = ty.type_visibility;
    let generics = ty.generics;
    let t_default = || TokenStream::default();
    let namespace = ty
        .config_it_namespace
        .as_ref()
        .map_or(t_default(), |x| quote! { #x :: });
    let use_lazy_static = ty
        .config_it_namespace
        .as_ref()
        .map_or(t_default(), |x| quote! { use #x :: lazy_static; });

    let fields = replace(&mut ty.fields, Default::default());
    let num_fields = fields.len();
    let mut indexer = -1;
    let fields = fields.into_iter().map(|x| {
        let ident = x.identifier.to_string();
        let ty = x.src_type;
        let alias = x
            .alias
            .map_or_else(|| ident.clone(), |x| x.into_token_stream().to_string());
        let doc = x.docstring;
        let default = x.default_value.map_or_else(
            || Default::default(),
            |v| {
                quote! {
                        let default_value = #v as _;
                }
            },
        );

        let min = x.min.map_or(t_default(), |x| x.into_token_stream());
        let max = x.max.map_or(t_default(), |x| x.into_token_stream());
        let one_of = x.one_of.map_or(t_default(), |x| x.into_token_stream());

        let disable_import = x.flag_disable_import;
        let disable_export = x.flag_disable_export;
        let hidden = x.flag_hidden;

        indexer += 1;

        let meta_gen = quote! {
            {
                type Type = #ty;

                let offset = unsafe {
                    let owner = 0 as *const MyStruct;
                    &(*owner).#ident as *const _ as *const u8 as usize;
                };
                let identifier = #ident;
                let varname = #alias;
                let doc_string = #doc;
                let index = #indexer;

                // Optional args
                #default

                let init = MetadataValInit::<Type> {
                    fn_validate: |_, _| -> Option<bool> { Some(true) },
                    v_default: default_value,
                    v_one_of: [#one_of],
                    v_max: #min,
                    v_min: #max,
                };

                let props = MetadataProps {
                    description: doc_string,
                    varname,
                    disable_import: #disable_import,
                    disable_export: #disable_export,
                    hidden: #hidden,
                };

                let meta = Metadata::create_for_base_type(identifier, init, props);

                let prop_data = PropData {
                    index,
                    type_id: TypeId::of::<Type>(),
                    meta: Arc::new(meta),
                };

                s.insert(offset, prop_data);
            }
        };

        (meta_gen, x.identifier)
    });

    let mut vec_fields = Vec::with_capacity(num_fields);
    let mut vec_idents = Vec::with_capacity(num_fields);

    fields.for_each(|(a, b)| {
        vec_fields.push(a);
        vec_idents.push(b);
    });

    Ok(quote! {
        impl #generics config_it::ConfigGroupData for #identifier #generics {
            fn prop_desc_table__() -> &'static std::collections::HashMap<usize, PropData> {
                #use_lazy_static

                lazy_static! {
                    static ref TABLE: Arc<std::collections::HashMap<usize, PropData>> = {
                        let mut s = HashMap::new();

                        #(#vec_fields)*

                        Arc::new(s)
                    };
                }

                &*TABLE
            }
        }

        fn elem_at_mut__(&mut self, index: usize) -> &mut dyn std::any::Any {
            todo!()
        }
    })
}
