use std::collections::HashMap;

use codemap::Span;
use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::{mapped_arena::MappedArena, primitive_type::PrimitiveType};
use core_model_builder::{
    ast::ast_types::{AstModel, AstModelKind, AstService, AstSystem, Untyped},
    typechecker::{
        annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParamSpec},
        typ::{Service, Type, TypecheckedSystem},
        Scope,
    },
};
use core_plugin_interface::interface::SubsystemBuilder;

use crate::error::ParserError;

mod annotation;
pub mod annotation_map;
mod annotation_params;
mod expression;
mod field;
mod field_default_value;
mod field_type;
mod logical_op;
mod model;
mod relational_op;
mod selection;
mod service;

pub trait TypecheckInto<T> {
    fn shallow(&self) -> T;
    fn pass(
        &self,
        typ: &mut T,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool;
}

pub trait TypecheckFrom<T>
where
    Self: Sized,
{
    fn shallow(untyped: &T) -> Self;
    fn pass(
        &mut self,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        scope: &Scope,
        errors: &mut Vec<Diagnostic>,
    ) -> bool;
}

fn populate_type_env(env: &mut MappedArena<Type>) {
    // TODO: maybe we don't need to do this manually
    env.add("Boolean", Type::Primitive(PrimitiveType::Boolean));
    env.add("Int", Type::Primitive(PrimitiveType::Int));
    env.add("Float", Type::Primitive(PrimitiveType::Float));
    env.add("Decimal", Type::Primitive(PrimitiveType::Decimal));
    env.add("String", Type::Primitive(PrimitiveType::String));
    env.add("LocalTime", Type::Primitive(PrimitiveType::LocalTime));
    env.add(
        "LocalDateTime",
        Type::Primitive(PrimitiveType::LocalDateTime),
    );
    env.add("LocalDate", Type::Primitive(PrimitiveType::LocalDate));
    env.add("Instant", Type::Primitive(PrimitiveType::Instant));
    env.add("Json", Type::Primitive(PrimitiveType::Json));
    env.add("Blob", Type::Primitive(PrimitiveType::Blob));
    env.add("Uuid", Type::Primitive(PrimitiveType::Uuid));

    env.add("Claytip", Type::Primitive(PrimitiveType::Claytip));
    env.add("ClaytipPriv", Type::Primitive(PrimitiveType::ClaytipPriv));

    env.add(
        "Operation",
        Type::Primitive(PrimitiveType::Interception("Operation".to_string())),
    );
}

fn populate_annotation_env(
    subsystem_builders: &[Box<dyn SubsystemBuilder>],
    env: &mut HashMap<String, AnnotationSpec>,
) {
    let mut annotations = vec![
        (
            "access",
            AnnotationSpec {
                targets: &[
                    AnnotationTarget::Type,
                    AnnotationTarget::Field,
                    AnnotationTarget::Method,
                ],
                no_params: false,
                single_params: true,
                mapped_params: Some(&[
                    MappedAnnotationParamSpec {
                        name: "query",
                        optional: true,
                    },
                    MappedAnnotationParamSpec {
                        name: "mutation",
                        optional: true,
                    },
                    MappedAnnotationParamSpec {
                        name: "create",
                        optional: true,
                    },
                    MappedAnnotationParamSpec {
                        name: "update",
                        optional: true,
                    },
                    MappedAnnotationParamSpec {
                        name: "delete",
                        optional: true,
                    },
                ]),
            },
        ),
        (
            "cookie",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "env",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
        #[cfg(feature = "test-context")]
        (
            "test",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "before",
            AnnotationSpec {
                targets: &[AnnotationTarget::Interceptor],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "after",
            AnnotationSpec {
                targets: &[AnnotationTarget::Interceptor],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "around",
            AnnotationSpec {
                targets: &[AnnotationTarget::Interceptor],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "header",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "inject",
            AnnotationSpec {
                targets: &[AnnotationTarget::Argument],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "clientIp",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: false,
                mapped_params: None,
            },
        ),
        (
            "jwt",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "query",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
    ];

    for builder in subsystem_builders.iter() {
        annotations.extend(builder.annotations())
    }

    for (name, spec) in annotations {
        env.insert(name.to_owned(), spec);
    }
}

pub fn build(
    subsystem_builders: &[Box<dyn SubsystemBuilder>],
    ast_system: AstSystem<Untyped>,
) -> Result<TypecheckedSystem, ParserError> {
    let mut types_arena: MappedArena<Type> = MappedArena::default();
    let mut services_arena: MappedArena<Service> = MappedArena::default();
    let mut annotation_env = HashMap::new();
    populate_type_env(&mut types_arena);
    populate_annotation_env(subsystem_builders, &mut annotation_env);

    validate_no_duplicates(&ast_system.services, |s| &s.name, |s| s.span, "service")?;

    let mut ast_service_types: Vec<AstModel<Untyped>> = vec![];
    for service in ast_system.services.iter() {
        ast_service_types.extend(service.types.clone());
        services_arena.add(&service.name, Service(AstService::shallow(service)));

        validate_service(service)?;
    }

    let ast_types_iter = ast_system.types.iter().chain(ast_service_types.iter());
    let ast_root_types = &ast_system.types;

    for ast_type in ast_types_iter.clone() {
        types_arena.add(
            ast_type.name.as_str(),
            Type::Composite(AstModel::shallow(ast_type)),
        );
    }

    loop {
        let mut did_change = false;
        let init_scope = Scope {
            enclosing_type: None,
        };

        let mut errors = Vec::new();

        for ast_root_type in ast_root_types.iter() {
            let mut typ = types_arena
                .get_by_key(ast_root_type.name.as_str())
                .unwrap()
                .clone();
            if let Type::Composite(c) = &mut typ {
                if c.kind != AstModelKind::Context {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: "Models and types are not permitted outside a service".to_string(),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: ast_root_type.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    })
                }
            }
        }

        // Temporary workaround to avoid reporting errors twice (once here, and once in the service pass)
        // TODO: Remove this after fixing https://github.com/payalabs/payas/issues/596
        let mut ignore_errors = Vec::new();

        for ast_type in ast_types_iter.clone() {
            let mut typ = types_arena
                .get_by_key(ast_type.name.as_str())
                .unwrap()
                .clone();
            if let Type::Composite(c) = &mut typ {
                let pass_res = c.pass(
                    &types_arena,
                    &annotation_env,
                    &init_scope,
                    &mut ignore_errors,
                );
                if pass_res {
                    *types_arena.get_by_key_mut(ast_type.name.as_str()).unwrap() = typ;
                    did_change = true;
                }
            } else {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: format!("Type {} is not a model", typ),
                    code: Some("C000".to_string()),
                    spans: vec![],
                });
            }
        }

        for ast_service in ast_system.services.iter() {
            let mut service = services_arena
                .get_by_key(ast_service.name.as_str())
                .unwrap()
                .clone();
            let Service(s) = &mut service;

            let pass_res = s.pass(&types_arena, &annotation_env, &init_scope, &mut errors);
            if pass_res {
                *services_arena
                    .get_by_key_mut(ast_service.name.as_str())
                    .unwrap() = service;
                did_change = true;
            }
        }

        if !did_change {
            if !errors.is_empty() {
                return Err(ParserError::Diagnosis(errors));
            } else {
                return Ok(TypecheckedSystem {
                    types: types_arena,
                    services: services_arena,
                });
            }
        }
    }
}

fn validate_service(service: &AstService<Untyped>) -> Result<(), ParserError> {
    validate_no_duplicates(
        &service.methods,
        |method| &method.name,
        |method| method.span,
        "operation",
    )?;

    validate_no_duplicates(
        &service.types,
        |model| &model.name,
        |model| model.span,
        "model/type",
    )
}

fn validate_no_duplicates<T>(
    items: &[T],
    get_name: impl Fn(&T) -> &str,
    get_span: impl Fn(&T) -> Span,
    item_kind: &str, // To print as a part of the error message
) -> Result<(), ParserError> {
    let mut items_with_pos = HashMap::new();
    let mut duplicates_with_pos = vec![];

    for item in items.iter() {
        // TODO: Use try_insert when it's stable. This way the first item will always be designated as the existing one
        // Currently, if we have three duplicates, we get diagnostics as (existing: 1, duplicate: 2), (existing: 2, duplicate: 3)
        let existing_item = items_with_pos.insert(get_name(item), get_span(item));

        if let Some(existing_span) = existing_item {
            duplicates_with_pos.push((get_name(item), existing_span, get_span(item)));
        }
    }

    if duplicates_with_pos.is_empty() {
        Ok(())
    } else {
        let diagnostics =
            duplicates_with_pos
                .into_iter()
                .map(|(name, existing_span, duplicate_span)| Diagnostic {
                    level: Level::Error,
                    message: format!("Duplicate {item_kind}: {name}"),
                    code: Some("C000".to_string()),
                    spans: vec![
                        SpanLabel {
                            span: existing_span,
                            style: SpanStyle::Primary,
                            label: Some("first defined here".to_string()),
                        },
                        SpanLabel {
                            span: duplicate_span,
                            style: SpanStyle::Secondary,
                            label: Some("again defined here".to_string()),
                        },
                    ],
                });

        Err(ParserError::Diagnosis(diagnostics.collect()))
    }
}
#[cfg(test)]
pub mod test_support {
    use codemap::CodeMap;

    use super::*;
    use crate::{load_subsystem_builders, parser::parse_str};

    pub fn build(src: &str) -> Result<TypecheckedSystem, ParserError> {
        let mut codemap = CodeMap::new();
        let parsed = parse_str(src, &mut codemap, "input.clay")?;
        let subsystem_builders = load_subsystem_builders().unwrap();
        super::build(&subsystem_builders, parsed)
    }

    pub fn parse_sorted(src: &str) -> Vec<(String, Type)> {
        let checked = build(src).unwrap();

        let mut entries: Vec<_> = checked
            .types
            .keys()
            .map(|key| (key.clone(), checked.types.get_by_key(key).unwrap().clone()))
            .collect();

        entries.sort_by_key(|pair| pair.0.to_owned());
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{build, parse_sorted};

    // Due to a change in insta version 1.12, test names (hence the snapshot names) get derived
    // from the surrounding function, so we must use a macro instead of a helper function.
    macro_rules! assert_typechecking {
        ($src:expr) => {
            insta::with_settings!({sort_maps => true}, {
                insta::assert_yaml_snapshot!(build($src).unwrap())
            });
        };
    }

    #[test]
    fn simple() {
        let src = r#"
        @postgres
        service UserService {
            type User {
              @column("custom_column") @access(self.role == "role_admin" || self.role == "role_superuser" || self.doc.is_public)
              doc: Doc;
              role: String
            }

            type Doc {
              is_public: Boolean
            }
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn with_auth_context_use_in_field_annotation() {
        let src = r#"
        context AuthContext {
            @jwt role: String
        }

        @postgres
        service DocumentService {
            type Doc {
              is_public: Boolean
              @access(AuthContext.role == "ROLE_ADMIN" || self.is_public) content: String 
            }
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn with_array_in_operator() {
        let src = r#"
        context AuthContext {
            @jwt roles: Array<String> 
        }

        @postgres
        service DocumentService {
            type Doc {
              @access("ROLE_ADMIN" in AuthContext.roles) content: String 
            }
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn with_auth_context_use_in_type_annotation() {
        let src = r#"
        context AuthContext {
            @jwt role: String
        }
        
        @postgres
        service DocumentService {
            @access(AuthContext.role == "ROLE_ADMIN" || self.is_public)
            type Doc {
              is_public: Boolean
              content: String
            }
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn insignificant_whitespace() {
        let typical = r#"
        @postgres
        service DocumentService {
            @table("venues")
            type Venue {
                @column("idx") @pk id: Int 
                name: String
            }
        }
        "#;

        let with_whitespace = r#"
        @postgres
        service      DocumentService{
        @table ( "venues" )
        type    Venue
        {
            @column(  "idx"  )    @pk  id:   Int   
           

            name:String

        }}

        "#;

        let typical_parsed = serde_json::to_string(&parse_sorted(typical)).unwrap();
        let with_whitespace_parsed = serde_json::to_string(&parse_sorted(with_whitespace)).unwrap();
        assert_eq!(typical_parsed, with_whitespace_parsed);
    }

    #[test]
    fn unknown_annotation() {
        let src = r#"
        @postgres
        service UserService {
            @asdf
            type User {
            }
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn duplicate_annotation() {
        let src = r#"
        @postgres
        service UserService {
            @table("users")
            @table("users")
            type User {
            }
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn duplicate_plugin_annotations() {
        let src = r#"
        @postgres
        @postgres
        service UserService {
            @table("users")
            type User {
            }
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn no_plugin_annotation() {
        let src = r#"
        service UserService {
            type User {
            }
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn type_at_root() {
        let src_model = r#"
        type User {
        }
        "#;

        assert_err(src_model);
    }

    #[test]
    fn context_in_a_service() {
        let src_model = r#"
        @postgres
        service UserService {
            context AuthContext {
                @jwt role: String
            }
        }
        "#;

        assert_err(src_model);
    }

    #[test]
    fn invalid_annotation_parameter_type() {
        let expected_none = r#"
        @postgres
        service UserService {
            type User {
                @pk("asdf") id: Int 
            }
        }
        "#;

        let expected_single = r#"
        @postgres
        service UserService {
            @table
            type User {
            }
        }
        "#;

        let expected_map = r#"
        @postgres
        service UserService {
            type User {
                @range(5) id: Int
            }
        }
        "#;

        assert_err(expected_none);
        assert_err(expected_single);
        assert_err(expected_map);
    }

    #[test]
    fn duplicate_annotation_mapped_param() {
        let src = r#"
        @postgres
        service UserService {
            type User {
                @range(min=5, max=10, min=3) id: Int
            }
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn unknown_annotation_mapped_param() {
        let src = r#"
        @postgres
        service UserService {
            type User {
                @range(min=5, maxx=10) id: Int
            }
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn invalid_annotation_target() {
        let model = r#"
        @postgres
        service UserService {
            @pk
            type User {
            }
        }
        "#;

        let field = r#"
        @postgres
        service UserService {
            type User {
                @table("asdf") id: Int
            }
        }
        "#;

        assert_err(model);
        assert_err(field);
    }

    #[test]
    fn multiple_types() {
        let model = r#"
        @deno("test.ts")
        service User {
            type User {
                id: Int
            }

            type User {
                id: Int
            }

            query userName(id: Int): String
        }
        "#;

        assert_err(model);
    }

    #[test]
    fn multiple_same_named_services() {
        let model = r#"
        @deno("foo.js")
        service Foo {
            query userName(id: Int): String
        }

        @deno("foo.js")
        service Foo {
            query userName(id: Int): String
        }
        "#;

        assert_err(model);
    }

    #[test]
    fn multiple_same_named_operations() {
        let model = r#"
        @deno("foo.js")
        service Foo {
            query userName(id1: Int): String
            query userName(id2: Int): String
            query userName(id3: Int): String
        }
        "#;

        assert_err(model);
    }

    #[test]
    fn multiple_same_named_typess() {
        let model = r#"
        @deno("foo.js")
        service Foo {
            type User {
                id: Int
                name: String
            }
            type User {
                id: Int
                name: String
            }
        }
        "#;

        assert_err(model);
    }

    #[test]
    fn multiple_same_named_types() {
        let model = r#"
        @postgres
        service Foo {
            type User {
                id: Int
                name: String
            }
            type User {
                id: Int
                name: String
            }
        }
        "#;

        assert_err(model);
    }

    #[test]
    fn multiple_same_named_model_and_types() {
        let model = r#"
        @deno("foo.js")
        service Foo {
            type User {
                id: Int
                name: String
            }
            type User {
                id: Int
                name: String
            }
        }
        "#;

        assert_err(model);
    }

    fn assert_err(src: &str) {
        assert!(build(src).is_err());
    }
}
