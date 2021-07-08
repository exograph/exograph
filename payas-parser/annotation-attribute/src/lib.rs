use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, AttributeArgs, Field, Fields, Ident, ItemEnum, Lit, NestedMeta, Type,
};

use std::collections::HashSet;

/// Annotation for creating a claytip annotation with type-checked parameters.
///
/// The annotation enum can contain any of three variants: `None`, `Single`, and
/// `Map`. `Single` must be a tuple struct with a single parameter. `Map` must
/// be a struct with named fields. Fields in `Map` may also be optional by
/// using `Option`.
///
/// # Generated methods
///
/// `from_params` - Constructs the annotation given the `TypedAnnotationParams`.
///
/// `pass` - Performs a type-check pass on all the parameters.
///
/// `value` - If the annotation only contains the `Single` field, `value`
/// returns the single value. If the annotation only contains the `None` and
/// `Single` field, `value` returns an `Option` instead.
///
/// `name` - Returns the name of the annotation in model files (given as an
/// argument to attribute)
#[proc_macro_attribute]
pub fn annotation(args: TokenStream, input: TokenStream) -> TokenStream {
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

        let mut variant = |name: &'static str| {
            let name = name.to_string();
            let allows = variant_names.contains(&name);

            if allows {
                variant_names.remove(&name);
                Some(*enum_variants.iter().find(|v| v.ident == name).unwrap())
            } else {
                None
            }
        };

        let none = variant("None");
        let single = variant("Single");
        let map = variant("Map");

        // If there are any variants other than None, Single, or Map
        if !variant_names.is_empty() {
            panic!("Only None, Single, and Map variants allowed");
        }

        (none, single, map)
    };

    // Build from_params function
    let from_params_fn = {
        // If given no parameters
        let from_none = if none_variant.is_some() {
            quote! { Ok(Self::None) }
        } else {
            quote! { Err(vec!["expected parameters".to_string()]) }
        };

        // If given a single parameter
        let from_single = if single_variant.is_some() {
            quote! { Ok(Self::Single(expr)) }
        } else {
            quote! { Err(vec!["unexpected unnamed parameter".to_string()]) }
        };

        // If given a map of parameters
        let from_map = if let Some(variant) = map_variant {
            // (field, field is_optional)
            let fields = if let Fields::Named(fields) = &variant.fields {
                fields
                    .named
                    .iter()
                    .map(|field| (field, is_optional(&field)))
                    .collect::<Vec<_>>()
            } else {
                panic!("Map must have named parameters");
            };

            // (field name, field is_optional)
            let (expected_fields, expected_fields_is_optional): (Vec<String>, Vec<bool>) = fields
                .iter()
                .map(|(field, is_optional)| {
                    (field.ident.as_ref().unwrap().to_string(), is_optional)
                })
                .unzip();

            // Build the annotation constructor
            let constructor = fields
                .iter()
                .map(|(field, is_optional)| {
                    let ident = field.ident.as_ref().unwrap();
                    let n = ident.to_string();

                    if *is_optional {
                        quote! { #ident: params.get(#n).map(|p| p.clone()) }
                    } else {
                        quote! { #ident: params.get(#n).unwrap().clone() }
                    }
                })
                .collect::<Vec<_>>();

            quote! {
                let mut errs = Vec::new();

                // Keep track of extra unused parameters
                let mut unexpected_params = params.keys().cloned().collect::<std::collections::HashSet<_>>();

                // For each field, check if it is given or if it's optional
                for (expected, is_optional) in [#((#expected_fields, #expected_fields_is_optional)),*] {
                    if params.contains_key(expected) {
                        unexpected_params.remove(expected);
                    } else if !is_optional {
                        errs.push(format!("Expected parameters {}", expected));
                    }
                }

                // For any unexpected parameters, push an error
                for unexpected in unexpected_params {
                    errs.push(format!("Unexpected parameter {}", unexpected));
                }

                if errs.is_empty() {
                    Ok(Self::Map {
                        #(#constructor,)*
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
        // If given no parameters, don't need a pass function (always false)

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
            let (idents, is_optionals): (Vec<&Ident>, Vec<bool>) =
                if let Fields::Named(fields) = &variant.fields {
                    fields
                        .named
                        .iter()
                        .map(|field| (field.ident.as_ref().unwrap(), is_optional(&field)))
                        .unzip()
                } else {
                    panic!("Map must have named parameters");
                };

            let passes = idents
                .iter()
                .zip(is_optionals)
                .map(|(ident, is_optional)| {
                    let n = ident.to_string();

                    if is_optional {
                        quote! {
                            if let Some(#ident) = #ident {
                                let param_changed = ast_params[#n].pass(#ident, env, scope, errors);
                                changed = changed || param_changed;
                            }
                        }
                    } else {
                        quote! {
                            let param_changed = ast_params[#n].pass(#ident, env, scope, errors);
                            changed = changed || param_changed;
                        }
                    }
                })
                .collect::<Vec<_>>();

            quote! {
                if let Self::Map { #(#idents),* } = self {
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

/// Checks if a field is optional (if the type is `Option`).
fn is_optional(field: &Field) -> bool {
    if let Type::Path(ty) = &field.ty {
        let segments = ty.path.segments.iter().collect::<Vec<_>>();
        segments.last().unwrap().ident == "Option"
    } else {
        panic!("Type must be TypedExpression or Option<TypedExpression>");
    }
}
