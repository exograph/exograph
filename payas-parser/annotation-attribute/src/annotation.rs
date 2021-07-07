use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, AttributeArgs, Fields, ItemEnum, Lit, NestedMeta, Variant};

// TODO documentation
pub(crate) fn annotation(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as AttributeArgs);
    let input = parse_macro_input!(input as ItemEnum);

    let enum_name = &input.ident;
    let enum_variants = input.variants.iter().collect::<Vec<_>>();

    // Get info about None/Single/Map variants
    let (none_variant, single_variant, map_variant) = {
        let mut variant_names = enum_variants
            .iter()
            .map(|v| v.ident.to_string())
            .collect::<HashSet<_>>();

        fn allows(variants: &mut HashSet<String>, name: &'static str) -> bool {
            let name = name.to_string();
            let allows = variants.contains(&name);
            if allows {
                variants.remove(&name);
            }
            allows
        }

        fn variant<'a>(
            variants: &'a [&Variant],
            names: &mut HashSet<String>,
            name: &'static str,
        ) -> Option<&'a Variant> {
            if allows(names, name) {
                Some(*variants.iter().find(|v| v.ident == name).unwrap())
            } else {
                None
            }
        }

        let none = variant(&enum_variants, &mut variant_names, "None");
        let single = variant(&enum_variants, &mut variant_names, "Single");
        let map = variant(&enum_variants, &mut variant_names, "Map");

        if !variant_names.is_empty() {
            panic!("Only None, Single, and Map variants allowed");
        }

        (none, single, map)
    };

    // Build from_params function
    let from_params_fn = {
        let from_none = if none_variant.is_some() {
            quote! { Ok(Self::None) }
        } else {
            quote! { Err(vec!["expected parameters".to_string()]) }
        };

        let from_single = if single_variant.is_some() {
            quote! { Ok(Self::Single(expr)) }
        } else {
            quote! { Err(vec!["unexpected unnamed parameter".to_string()]) }
        };

        let from_map = if let Some(variant) = map_variant {
            let field_idents = if let Fields::Named(fields) = &variant.fields {
                fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect::<Vec<_>>()
            } else {
                panic!("Map must have named parameters");
            };

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
                let mut errs = Vec::new();

                // Keep track of extra unused parameters
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
                    Ok(Self::Map {
                        #(#constructor_fields,)*
                    })
                } else {
                    Err(errs)
                }
            }
        } else {
            quote! { Err(vec!["unexpected parameters".to_string()]) }
        };

        quote! {
            fn from_params(params: TypedAnnotationParams) -> Result<Self, Vec<String>> {
                match params {
                    TypedAnnotationParams::None => { #from_none },
                    TypedAnnotationParams::Single(expr) => { #from_single },
                    TypedAnnotationParams::Map(params) => { #from_map },
                }
            }
        }
    };

    // Build pass function
    let pass_fn = {
        let pass_single = if single_variant.is_some() {
            quote! {
                if let Self::Single(expr) = self {
                    ast_expr.pass(expr, env, scope, errors)
                } else {
                    panic!();
                }
            }
        } else {
            quote! { panic!(); }
        };

        let pass_map = if let Some(variant) = map_variant {
            let field_idents = if let Fields::Named(fields) = &variant.fields {
                fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect::<Vec<_>>()
            } else {
                panic!("Map must have named parameters");
            };

            let passes = field_idents
                .iter()
                .map(|i| {
                    let n = i.to_string();
                    quote! {
                        let param_changed = ast_params[#n].pass(#i, env, scope, errors);
                        changed = changed || param_changed;
                    }
                })
                .collect::<Vec<_>>();

            quote! {
                if let Self::Map { #(#field_idents),* } = self {
                    let mut changed = false;
                    #(#passes)*
                    changed
                } else {
                    panic!();
                }
            }
        } else {
            quote! { panic!(); }
        };

        quote! {
            fn pass(
                &mut self,
                params: &AstAnnotationParams,
                env: &MappedArena<Type>,
                scope: &Scope,
                errors: &mut Vec<codemap_diagnostic::Diagnostic>
            ) -> bool {
                match params {
                    AstAnnotationParams::None => false,
                    AstAnnotationParams::Single(ast_expr) => { #pass_single }
                    AstAnnotationParams::Map(ast_params) => { #pass_map }
                }
            }
        }
    };

    // Build value function
    // If the annotation only has a `Single` variant, `value()` returns the single value
    // If the annotation has `None` and `Single`, `value()` returns an optional
    let value_fn = if none_variant.is_none() && single_variant.is_some() && map_variant.is_none() {
        quote! {
            pub fn value(&self) -> &TypedExpression {
                match &self {
                    Self::Single(value) => value,
                    _ => panic!(),
                }
            }
        }
    } else if none_variant.is_some() && single_variant.is_some() && map_variant.is_none() {
        quote! {
            pub fn value(&self) -> Option<&TypedExpression> {
                match &self {
                    Self::None => None,
                    Self::Single(value) => Some(value),
                    _ => panic!(),
                }
            }
        }
    } else {
        quote! {}
    };

    // Get claytip annotation name from the annotation arguments
    if args.len() != 1 {
        panic!("expected claytip name literal");
    }

    let claytip_name = if let NestedMeta::Lit(l) = args.first().unwrap() {
        if let Lit::Str(l) = l {
            l.value()
        } else {
            panic!("expected string literal");
        }
    } else {
        panic!("expected literal");
    };

    // Build annotation output
    let expanded = quote! {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #input

        impl #enum_name {
            #from_params_fn

            #pass_fn

            #value_fn

            fn name() -> &'static str {
                #claytip_name
            }
        }
    };

    TokenStream::from(expanded)
}
