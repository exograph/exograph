use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, AttributeArgs, Fields, ItemStruct};

use crate::name_fn;

pub(crate) fn annotation(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as AttributeArgs);
    let input = parse_macro_input!(input as ItemStruct);

    let attrs = input.attrs;
    let vis = input.vis;
    let name = input.ident;
    let fields = input.fields;

    let (annot_struct, from_params_fn, pass_fn) = match fields {
        Fields::Unit => {
            let annot_struct = quote! {
                #(#attrs)*
                #vis struct #name;
            };

            // If the annotation struct has no fields, check there are no parameters
            let from_params_fn = quote! {
                fn from_params(params: TypedAnnotationParams) -> Result<Self, Vec<String>> {
                    if let TypedAnnotationParams::None = params {
                        Ok(Self {})
                    } else {
                        Err(vec!["expected no parameters".to_string()])
                    }
                }
            };

            // No args - nothing to check
            let pass_fn = quote! {};

            (annot_struct, from_params_fn, pass_fn)
        }
        Fields::Unnamed(fields) => {
            // There may only be one unnamed parameter
            if fields.unnamed.len() != 1 {
                panic!("Annotation may only have one unnamed parameter");
            }

            let annot_struct = {
                let field = fields.unnamed.first().unwrap();
                let field_type = &field.ty;
                let field_vis = &field.vis;

                quote! {
                    #(#attrs)*
                    #vis struct #name(#field_vis #field_type);
                }
            };

            let from_params_fn = quote! {
                fn from_params(params: TypedAnnotationParams) -> Result<Self, Vec<String>> {
                    if let TypedAnnotationParams::Single(expr) = params {
                        Ok(Self(expr))
                    } else {
                        Err(vec!["expected an unnamed parameter".to_string()])
                    }
                }
            };

            let pass_fn = quote! {
                fn pass(&mut self, expr: &AstExpr, env: &MappedArena<Type>, scope: &Scope, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> bool {
                    expr.pass(&mut self.0, env, scope, errors)
                }
            };

            (annot_struct, from_params_fn, pass_fn)
        }
        Fields::Named(fields) => {
            let field_idents = fields
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .collect::<Vec<_>>();

            let annot_struct = {
                let field_types = fields.named.iter().map(|f| &f.ty).collect::<Vec<_>>();
                let field_vis = fields.named.iter().map(|f| &f.vis).collect::<Vec<_>>();

                quote! {
                    #(#attrs)*
                    #vis struct #name {
                        #(#field_vis #field_idents: #field_types,)*
                    }
                }
            };

            let from_params_fn = {
                let expected_fields = field_idents
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>();

                let constructor_fields = field_idents
                    .iter()
                    .map(|i| {
                        let n = i.to_string();
                        quote! { #i: params[#n].clone() }
                    })
                    .collect::<Vec<_>>();

                quote! {
                    fn from_params(params: TypedAnnotationParams) -> Result<Self, Vec<String>> {
                        let mut errs = Vec::new();
                        if let TypedAnnotationParams::Map(params) = params {
                            // As we check parameters from TypedAnnotationParams, keep track of extra unused
                            // parameter
                            let mut unexpected_params = params.keys().cloned().collect::<std::collections::HashSet<_>>();

                            // For each field in the annotation struct, check if the parameter map contains
                            // the field by name
                            for expected in [#(#expected_fields),*] {
                                if params.contains_key(expected) {
                                    unexpected_params.remove(expected);
                                } else {
                                    errs.push(format!("Expected parameters {}", expected));
                                }
                            }

                            // For any unexpected parameters, push an error
                            for unexpected in unexpected_params {
                                errs.push(format!("Unexpected parameter {}", unexpected));
                            }

                            if errs.is_empty() {
                                Ok(Self {
                                    #(#constructor_fields,)*
                                })
                            } else {
                                Err(errs)
                            }
                        } else {
                            Err(vec!["expected named parameters".to_string()])
                        }
                    }
                }
            };

            let pass_fn = {
                let passes = field_idents
                    .iter()
                    .map(|i| {
                        let n = i.to_string();
                        quote! { pass = pass || params[#n].pass(&mut self.#i, env, scope, errors); }
                    })
                    .collect::<Vec<_>>();

                quote! {
                    fn pass(&mut self, params: &std::collections::HashMap<String, AstExpr>, env: &MappedArena<Type>, scope: &Scope, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> bool {
                        let mut pass = false;
                        #(#passes)*
                        pass
                    }
                }
            };

            (annot_struct, from_params_fn, pass_fn)
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
