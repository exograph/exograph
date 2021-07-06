use proc_macro::TokenStream;
use quote::quote;
use syn::{Field, Lit, NestedMeta, Type};

mod annotation;
mod unchecked_annotation;

#[proc_macro_attribute]
pub fn annotation(args: TokenStream, input: TokenStream) -> TokenStream {
    annotation::annotation(args, input)
}

#[proc_macro_attribute]
pub fn unchecked_annotation(args: TokenStream, input: TokenStream) -> TokenStream {
    unchecked_annotation::unchecked_annotation(args, input)
}

fn name_fn(args: &[NestedMeta]) -> proc_macro2::TokenStream {
    if args.len() != 1 {
        panic!("expected claytip name literal");
    }

    let name = if let NestedMeta::Lit(l) = args.first().unwrap() {
        if let Lit::Str(l) = l {
            l.value()
        } else {
            panic!("expected string literal");
        }
    } else {
        panic!("expected literal");
    };

    quote! {
        pub const fn name() -> &'static str {
            #name
        }

        // TODO this is dumb
        pub const fn name2(&self) -> &'static str {
            Self::name()
        }
    }
}

// TODO also verify if field is TypedExpression
fn is_optional(field: &Field) -> bool {
    if let Type::Path(ty) = &field.ty {
        let segments = ty.path.segments.iter().collect::<Vec<_>>();
        segments.last().unwrap().ident == "Option"
    } else {
        panic!("Type must be TypedExpression or Option<TypedExpression>");
    }
}
