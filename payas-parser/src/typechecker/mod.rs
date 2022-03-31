mod annotation;
mod annotation_map;
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
mod typ;

use std::collections::HashMap;

use codemap_diagnostic::Diagnostic;
use serde::{Deserialize, Serialize};

pub(super) use annotation_map::AnnotationMap;

pub(super) use typ::{PrimitiveType, Type};

use crate::ast::ast_types::{AstModel, AstService, NodeTypedness};
use crate::ast::ast_types::{AstSystem, Untyped};
use crate::error::ParserError;
use payas_model::model::mapped_arena::MappedArena;

use self::annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParamSpec};

pub struct Scope {
    pub enclosing_model: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Typed;
impl NodeTypedness for Typed {
    type FieldSelection = Type;
    type RelationalOp = Type;
    type Expr = Type;
    type LogicalOp = Type;
    type Field = Type;
    type Annotations = AnnotationMap;
    type Type = bool;
}

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

    env.add("Claytip", Type::Primitive(PrimitiveType::ClaytipInjected));

    env.add(
        "Operation",
        Type::Primitive(PrimitiveType::Interception("Operation".to_string())),
    );
}

fn populate_annotation_env(env: &mut HashMap<String, AnnotationSpec>) {
    let annotations = [
        (
            "access",
            AnnotationSpec {
                targets: &[
                    AnnotationTarget::Model,
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
            "bits",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "column",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "dbtype",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
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
        (
            "external",
            AnnotationSpec {
                targets: &[AnnotationTarget::Service],
                no_params: false,
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
            "jwt",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "length",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "pk",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: false,
                mapped_params: None,
            },
        ),
        (
            "plural_name",
            AnnotationSpec {
                targets: &[AnnotationTarget::Model],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "precision",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "range",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: false,
                mapped_params: Some(&[
                    MappedAnnotationParamSpec {
                        name: "min",
                        optional: false,
                    },
                    MappedAnnotationParamSpec {
                        name: "max",
                        optional: false,
                    },
                ]),
            },
        ),
        (
            "scale",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "size",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "table",
            AnnotationSpec {
                targets: &[AnnotationTarget::Model],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        ),
        (
            "unique",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: true,
                single_params: true,
                mapped_params: None,
            },
        ),
    ];

    for (name, spec) in annotations {
        env.insert(name.to_owned(), spec);
    }
}

pub fn build(ast_system: AstSystem<Untyped>) -> Result<MappedArena<Type>, ParserError> {
    let mut ast_service_models: Vec<AstModel<Untyped>> = vec![];

    let mut types_arena: MappedArena<Type> = MappedArena::default();
    let mut annotation_env = HashMap::new();
    populate_type_env(&mut types_arena);
    populate_annotation_env(&mut annotation_env);

    for service in ast_system.services.iter() {
        ast_service_models.extend(service.models.clone());
        types_arena.add(&service.name, Type::Service(AstService::shallow(service)));
    }

    let ast_types = [ast_system.models.as_slice(), ast_service_models.as_slice()].concat();
    let ast_services = ast_system.services;

    for model in ast_types.iter() {
        types_arena.add(
            model.name.as_str(),
            Type::Composite(AstModel::shallow(model)),
        );
    }

    loop {
        let mut did_change = false;
        let init_scope = Scope {
            enclosing_model: None,
        };

        let mut errors = Vec::new();

        for model in ast_types.iter() {
            let mut typ = types_arena.get_by_key(model.name.as_str()).unwrap().clone();
            if let Type::Composite(c) = &mut typ {
                let pass_res = c.pass(&types_arena, &annotation_env, &init_scope, &mut errors);
                if pass_res {
                    *types_arena.get_by_key_mut(model.name.as_str()).unwrap() = typ;
                    did_change = true;
                }
            } else {
                panic!()
            }
        }

        for service in ast_services.iter() {
            let mut typ = types_arena
                .get_by_key(service.name.as_str())
                .unwrap()
                .clone();
            if let Type::Service(s) = &mut typ {
                let pass_res = s.pass(&types_arena, &annotation_env, &init_scope, &mut errors);
                if pass_res {
                    *types_arena.get_by_key_mut(service.name.as_str()).unwrap() = typ;
                    did_change = true;
                }
            } else {
                panic!()
            }
        }

        if !did_change {
            if !errors.is_empty() {
                return Err(ParserError::Diagnosis(errors));
            } else {
                return Ok(types_arena);
            }
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use crate::parser::parse_str;

    pub fn build(src: &str) -> Result<MappedArena<Type>, ParserError> {
        let parsed = parse_str(src, "input.clay")?;
        super::build(parsed)
    }

    pub fn parse_sorted(src: &str) -> Vec<(String, Type)> {
        let checked = build(src).unwrap();

        let mut entries: Vec<_> = checked
            .keys()
            .map(|key| (key.clone(), checked.get_by_key(key).unwrap().clone()))
            .collect();

        entries.sort_by_key(|pair| pair.0.to_owned());
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{build, parse_sorted};

    // Due to a change in insta version 1.12, test names (hence the snapshot names) get derived
    // from the surrounding method, so we must use a macro instead of a helper function.
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
        model User {
          doc: Doc @column("custom_column") @access(self.role == "role_admin" || self.role == "role_superuser" || self.doc.is_public)
          role: String
        }

        model Doc {
          is_public: Boolean
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn with_auth_context_use_in_field_annotation() {
        let src = r#"
        context AuthContext {
            role: String @jwt
        }

        model Doc {
          is_public: Boolean
          content: String @access(AuthContext.role == "ROLE_ADMIN" || self.is_public)
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn with_array_in_operator() {
        let src = r#"
        context AuthContext {
            roles: Array<String> @jwt
        }

        model Doc {
          content: String @access("ROLE_ADMIN" in AuthContext.roles)
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn with_auth_context_use_in_type_annotation() {
        let src = r#"
        context AuthContext {
            role: String @jwt
        }

        @access(AuthContext.role == "ROLE_ADMIN" || self.is_public)
        model Doc {
          is_public: Boolean
          content: String
        }
        "#;

        assert_typechecking!(src);
    }

    #[test]
    fn insignificant_whitespace() {
        let typical = r#"
        @table("venues")
        model Venue {
            id: Int @column("idx") @pk
            name: String
        }
        "#;

        let with_whitespace = r#"

        @table ( "venues" )
        model    Venue
        {
            id:   Int   @column(  "idx"  )
            @pk

            name:String

        }

        "#;

        let typical_parsed = serde_json::to_string(&parse_sorted(typical)).unwrap();
        let with_whitespace_parsed = serde_json::to_string(&parse_sorted(with_whitespace)).unwrap();
        assert_eq!(typical_parsed, with_whitespace_parsed);
    }

    #[test]
    fn unknown_annotation() {
        let src = r#"
        @asdf
        model User {
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn duplicate_annotation() {
        let src = r#"
        @table("users")
        @table("users")
        model User {
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn invalid_annotation_parameter_type() {
        let expected_none = r#"
        model User {
            id: Int @pk("asdf")
        }
        "#;

        let expected_single = r#"
        @table
        model User {
        }
        "#;

        let expected_map = r#"
        model User {
            id: Int @range(5)
        }
        "#;

        assert_err(expected_none);
        assert_err(expected_single);
        assert_err(expected_map);
    }

    #[test]
    fn duplicate_annotation_mapped_param() {
        let src = r#"
        model User {
            id: Int @range(min=5, max=10, min=3)
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn unknown_annotation_mapped_param() {
        let src = r#"
        model User {
            id: Int @range(min=5, maxx=10)
        }
        "#;

        assert_err(src);
    }

    #[test]
    fn invalid_annotation_target() {
        let model = r#"
        @pk
        model User {
        }
        "#;

        let field = r#"
        model User {
            id: Int @table("asdf")
        }
        "#;

        assert_err(model);
        assert_err(field);
    }

    fn assert_err(src: &str) {
        assert!(build(src).is_err());
    }
}
