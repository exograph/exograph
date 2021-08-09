use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, AttributeArgs, Field, Fields, Ident, ItemEnum, Lit, NestedMeta, Type,
    Variant,
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

    let enum_name = &input.ident;
    let enum_variants = AnnotVariants::from(&input.variants.iter().collect::<Vec<_>>());

    let from_params_fn = build_from_params_fn(&claytip_name, &enum_variants);
    let pass_fn = build_pass_fn(&enum_variants);
    let value_fn = build_value_fn(&enum_variants);

    TokenStream::from(quote! {
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
    })
}

/// Data about the three annotation variants (None, Single, Map).
struct AnnotVariants<'a> {
    /// Does `None` variant exist
    none: bool,
    /// Does `Single` variant exist
    single: bool,
    /// If `Map` variant exists, vector of (field, field is optional)
    map: Option<Vec<(&'a Field, bool)>>,
}

impl<'a> AnnotVariants<'a> {
    fn from(enum_variants: &[&'a Variant]) -> Self {
        let mut variant_names = enum_variants
            .iter()
            .map(|v| v.ident.to_string())
            .collect::<HashSet<_>>();

        let mut get_variant = |name: &'static str| {
            let name = name.to_string();
            let allows = variant_names.contains(&name);

            if allows {
                variant_names.remove(&name);
                Some(*enum_variants.iter().find(|v| v.ident == name).unwrap())
            } else {
                None
            }
        };

        let none = get_variant("None");
        let single = get_variant("Single");
        let map = get_variant("Map");

        // If there are any variants other than None, Single, or Map
        if !variant_names.is_empty() {
            panic!("Only None, Single, and Map variants allowed");
        }

        AnnotVariants {
            none: none.is_some(),
            single: single.is_some(),
            map: match map {
                Some(v) => match &v.fields {
                    Fields::Named(fields) => Some(
                        fields
                            .named
                            .iter()
                            .map(|field| (field, is_optional(field)))
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                },
                None => None,
            },
        }
    }
}

/// Build the `from_params` function, which constructs the annotation given the typed annotation
/// parameters.
fn build_from_params_fn(claytip_name: &str, variants: &AnnotVariants) -> proc_macro2::TokenStream {
    // Message for what parameters were expected in case of a type error
    let diagnostic_msg = {
        let mut expected = Vec::new();

        if variants.none {
            expected.push("no parameters".to_string());
        }
        if variants.single {
            expected.push("a single parameter".to_string());
        }
        if let Some(map_fields) = &variants.map {
            expected.push(format!(
                "({})",
                join_strings(
                    map_fields
                        .iter()
                        .map(|(f, is_optional)| format!(
                            "{}{}",
                            f.ident.as_ref().unwrap(),
                            if *is_optional { "?" } else { "" }
                        ))
                        .collect(),
                    None,
                )
            ));
        }

        format!("Expected {}", join_strings(expected, Some("or")))
    };

    let base_diagnostic = quote! {
        errors.push(Diagnostic {
            level: Level::Error,
            message: format!("Incorrect parameters for `{}`", #claytip_name),
            code: Some("A000".to_string()),
            spans: vec![
                SpanLabel {
                    span: ast_annot.span,
                    label: Some(#diagnostic_msg.to_string()),
                    style: SpanStyle::Primary,
                }
            ],
        });
        bail!("");
    };

    // If given no parameters
    let from_none = if variants.none {
        quote! { Ok(Self::None) }
    } else {
        base_diagnostic.clone()
    };

    // If given a single parameter
    let from_single = if variants.single {
        quote! { Ok(Self::Single(expr)) }
    } else {
        base_diagnostic.clone()
    };

    // If given a map of parameters
    let from_map = if let Some(map_fields) = &variants.map {
        // (field name, field is optional)
        let (expected_fields, expected_fields_is_optional): (Vec<String>, Vec<bool>) = map_fields
            .iter()
            .map(|(field, is_optional)| (field.ident.as_ref().unwrap().to_string(), is_optional))
            .unzip();

        // Build the annotation constructor
        let constructor = map_fields
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
            let mut err_labels = Vec::new();
            let mut missing_param = false;

            // Check for any duplicate parameters
            let mut param_spans: HashMap<String, Span>  = HashMap::new();
            match &ast_annot.params {
                AstAnnotationParams::Map(_, spans) => {
                    for (name, span) in spans.iter() {
                        if param_spans.contains_key(name) {
                            err_labels.push(
                                SpanLabel {
                                    span: span.clone(),
                                    label: Some(format!("`{}` redefined here", name)),
                                    style: SpanStyle::Primary,
                                }
                            );
                            err_labels.push(
                                SpanLabel {
                                    span: param_spans[name],
                                    label: Some(format!("`{}` previously defined here", name)),
                                    style: SpanStyle::Secondary,
                                }
                            );
                        } else {
                            param_spans.insert(name.clone(), span.clone());
                        }
                    }
                },
                _ => panic!(),
            };

            // Keep track of extra unused parameters
            let mut unexpected_params = params.keys().cloned().collect::<HashSet<_>>();

            // For each field, check if it is given or if it's optional
            for (expected, is_optional) in [#((#expected_fields, #expected_fields_is_optional)),*] {
                if params.contains_key(expected) {
                    unexpected_params.remove(expected);
                } else if !is_optional {
                    missing_param = true;
                }
            }

            // For any unexpected parameters, push an error
            for unexpected in unexpected_params {
                err_labels.push(
                    SpanLabel {
                        span: param_spans[&unexpected],
                        label: Some(format!("`{}` unexpected", unexpected)),
                        style: SpanStyle::Primary,
                    }
                );
            }

            if err_labels.is_empty() && !missing_param {
                Ok(Self::Map {
                    #(#constructor,)*
                })
            } else {
                err_labels.push(
                    SpanLabel {
                        span: ast_annot.span,
                        label: Some(#diagnostic_msg.to_string()),
                        style: SpanStyle::Primary,
                    }
                );
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: format!("Incorrect parameters for `{}` annotation", #claytip_name),
                    code: Some("A000".to_string()),
                    spans: err_labels,
                });
                bail!("");
            }
        }
    } else {
        base_diagnostic
    };

    quote! {
        fn from_params(ast_annot: &AstAnnotation<crate::ast::ast_types::Untyped>, params: TypedAnnotationParams, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<Self> {
            match params {
                TypedAnnotationParams::None => { #from_none },
                TypedAnnotationParams::Single(expr) => { #from_single },
                TypedAnnotationParams::Map(params) => { #from_map },
            }
        }
    }
}

/// Build the `pass` function, which performs a pass on every parameter of the annotation.
fn build_pass_fn(variants: &AnnotVariants) -> proc_macro2::TokenStream {
    let pass_none = if variants.none {
        quote! {
            Self::None => { false }
        }
    } else {
        quote! {}
    };

    let pass_single = if variants.single {
        quote! {
            Self::Single(expr) => {
                expr.pass(env, scope, errors)
            }
        }
    } else {
        quote! {}
    };

    let pass_map = if let Some(map_fields) = &variants.map {
        // (field name, field is optional)
        let (idents, is_optionals): (Vec<&Ident>, Vec<bool>) = map_fields
            .iter()
            .map(|(field, is_optional)| (field.ident.as_ref().unwrap(), is_optional))
            .unzip();

        let passes = idents
            .iter()
            .zip(is_optionals)
            .map(|(ident, is_optional)| {
                if is_optional {
                    quote! {
                        if let Some(#ident) = #ident {
                            let param_changed = #ident.pass(env, scope, errors);
                            changed = changed || param_changed;
                        }
                    }
                } else {
                    quote! {
                        let param_changed = #ident.pass(env, scope, errors);
                        changed = changed || param_changed;
                    }
                }
            })
            .collect::<Vec<_>>();

        quote! {
            Self::Map { #(#idents),* } => {
                let mut changed = false;
                #(#passes)*
                changed
            }
        }
    } else {
        quote! {}
    };

    quote! {
        fn pass(
            &mut self,
            env: &MappedArena<Type>,
            scope: &Scope,
            errors: &mut Vec<codemap_diagnostic::Diagnostic>
        ) -> bool {
            match self {
                #pass_none
                #pass_single
                #pass_map
            }
        }
    }
}

/// Build the `value` function, which returns the single value if `Single` is the only variant, or
/// returns an `Option` if `None` and `Single` are the only variants.
fn build_value_fn(variants: &AnnotVariants) -> proc_macro2::TokenStream {
    if !variants.none && variants.single && variants.map.is_none() {
        quote! {
            pub fn value(&self) -> &AstExpr<Typed> {
                match &self {
                    Self::Single(value) => value,
                    _ => panic!(),
                }
            }
        }
    } else if variants.none && variants.single && variants.map.is_none() {
        quote! {
            pub fn value(&self) -> Option<&AstExpr<Typed>> {
                match &self {
                    Self::None => None,
                    Self::Single(value) => Some(value),
                    _ => panic!(),
                }
            }
        }
    } else {
        quote! {}
    }
}

/// Checks if a field is optional (if the type is `Option`).
fn is_optional(field: &Field) -> bool {
    if let Type::Path(ty) = &field.ty {
        let segments = ty.path.segments.iter().collect::<Vec<_>>();
        segments.last().unwrap().ident == "Option"
    } else {
        panic!("Unexpected field type");
    }
}

/// Join strings together with commas and an optional separator before the last word.
///
/// e.g. `join_strings(vec!["a", "b", "c"], Some("or")) == "a, b, or c"`
fn join_strings(strs: Vec<String>, last_sep: Option<&'static str>) -> String {
    match strs.len() {
        1 => strs[0].to_string(),
        2 => match last_sep {
            Some(last_sep) => format!("{} {} {}", strs[0], last_sep, strs[1]),
            None => format!("{}, {}", strs[0], strs[1]),
        },
        _ => {
            let mut joined = String::new();
            for i in 0..strs.len() {
                joined.push_str(&strs[i]);
                if i < strs.len() - 1 {
                    joined.push_str(", ");
                }
                if i == strs.len() - 2 {
                    joined.push_str(last_sep.unwrap_or(""));
                }
            }
            joined
        }
    }
}
