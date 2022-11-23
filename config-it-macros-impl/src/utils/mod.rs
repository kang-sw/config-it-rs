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

    let fields = replace(&mut ty.fields, Default::default());
    let fields = fields.into_iter().map(|x| {
        let vis = x.visibility;
        let ident = x.identifier;
        let ty = x.src_type;
        let doc = x.docstring;

        quote! {
             let init = MetadataValInit::<i32> {
                            fn_validate: |_, _| -> Option<bool> { Some(true) },
                            v_default: 13,
                            v_one_of: Default::default(),
                            v_max: Default::default(),
                            v_min: Default::default(),
                        };

            let mut meta = Metadata::create_for_base_type("hello".into(), init);
            meta.name = "override-if-exist".into();
            meta.description = "Docstring may placed here".into();
            meta.hidden = false;
            meta.disable_import = false;
            meta.disable_export = false;

            s.insert(
                0usize,
                PropData {
                    index: 0,
                    type_id: TypeId::of::<i32>(),
                    meta: Arc::new(meta),
                },
            );
        }
    });

    Ok(quote! {
        #vis impl #generics ConfigGroupData for #identifier #generics {
            fn prop_desc_table__() -> &'static HashMap<usize, PropData> {
                lazy_static! {
                    static ref TABLE: Arc<HashMap<usize, PropData>> = {
                        let mut s = HashMap::new();



                        Arc::new(s)
                    };
                }

                &*TABLE
            }
        }
    })
}
