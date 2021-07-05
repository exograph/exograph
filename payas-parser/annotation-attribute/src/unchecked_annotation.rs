use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, AttributeArgs, Fields, ItemStruct};

use crate::name_fn;

pub fn unchecked_annotation(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as AttributeArgs);
    let input = parse_macro_input!(input as ItemStruct);

    let attrs = input.attrs;
    let vis = input.vis;
    let name = input.ident;
    let fields = input.fields;

    if fields != Fields::Unit {
        panic!("Raw annotation must be unit struct");
    }

    let annot_struct = quote! {
        #(#attrs)*
        #vis struct #name {
            pub params: std::collections::HashMap<String, TypedExpression>
        }
    };

    let from_params_fn = quote! {
        fn from_params(params: TypedAnnotationParams) -> Result<Self, Vec<String>> {
            let params = match params {
                TypedAnnotationParams::None => std::collections::HashMap::new(),
                TypedAnnotationParams::Single(expr) => vec![("value".to_string(), expr)]
                    .into_iter()
                    .collect(),
                TypedAnnotationParams::Map(params) => params
            };
            Ok(Self { params })
        }
    };

    let pass_fn = quote! {
        fn pass(&mut self, params: &AstAnnotationParams, env: &MappedArena<Type>, scope: &Scope, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> bool {
            match &params {
                AstAnnotationParams::None => false,
                AstAnnotationParams::Single(expr) => {
                    expr.pass(self.params.get_mut("value").unwrap(), env, scope, errors)
                }
                AstAnnotationParams::Map(params) => {
                    params
                        .iter()
                        .map(|(name, expr)| {
                            let typed_expr = self.params.get_mut(name).unwrap();
                            (name, expr.pass(typed_expr, env, scope, errors))
                        })
                        .filter(|(_, changed)| *changed)
                        .count()
                        > 0
                }
            }
        }
    };

    let name_fn = name_fn(&args);

    let expanded = quote! {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #annot_struct

        impl #name {
            #from_params_fn

            #pass_fn

            #name_fn
        }
    };

    TokenStream::from(expanded)
}
