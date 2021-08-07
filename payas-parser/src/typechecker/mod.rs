mod annotation;
mod annotation_map;
mod annotation_params;
mod expression;
mod field;
mod field_type;
mod logical_op;
mod model;
mod relational_op;
mod selection;
mod typ;

use anyhow::{anyhow, Result};
use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Emitter};
use serde::{Deserialize, Serialize};

pub(super) use annotation::*;
pub(super) use annotation_map::AnnotationMap;

pub(super) use field::TypedField;
pub(super) use typ::{CompositeType, CompositeTypeKind, PrimitiveType, Type};

use crate::ast::ast_types::NodeTypedness;
use crate::ast::ast_types::{AstSystem, Untyped};
use payas_model::model::mapped_arena::MappedArena;

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
}

pub trait Typecheck<T> {
    #[allow(clippy::result_unit_err)] // Use unit result since errors are tracked as a parameter
    fn shallow(&self, errors: &mut Vec<codemap_diagnostic::Diagnostic>) -> Result<T>;
    fn pass(
        &self,
        typ: &mut T,
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool;
}

fn populate_standard_env(env: &mut MappedArena<Type>) {
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
}

pub fn build(ast_system: AstSystem<Untyped>, codemap: CodeMap) -> Result<MappedArena<Type>> {
    let ast_types = &ast_system.models;

    let mut types_arena: MappedArena<Type> = MappedArena::default();
    populate_standard_env(&mut types_arena);

    let mut errors = Vec::new();
    let mut emitter = Emitter::stderr(ColorConfig::Always, Some(&codemap));

    for model in ast_types {
        match model.shallow(&mut errors) {
            Ok(typ) => {
                types_arena.add(model.name.as_str(), typ);
            }
            Err(_) => {
                assert!(!errors.is_empty());
                emitter.emit(&errors);
                return Err(anyhow!("Could not process input clay files"));
            }
        }
    }

    loop {
        let mut did_change = false;
        let init_scope = Scope {
            enclosing_model: None,
        };

        let mut errors = Vec::new();

        for model in ast_types {
            let orig = types_arena.get_by_key(model.name.as_str()).unwrap();
            let mut typ = types_arena.get_by_key(model.name.as_str()).unwrap().clone();
            let pass_res = model.pass(&mut typ, &types_arena, &init_scope, &mut errors);
            if pass_res {
                assert!(*orig != typ);
                *types_arena.get_by_key_mut(model.name.as_str()).unwrap() = typ;
                did_change = true;
            }
        }

        if !did_change {
            if !errors.is_empty() {
                emitter.emit(&errors);
                return Err(anyhow!("Could not process input clay files"));
            } else {
                return Ok(types_arena);
            }
        }
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use crate::parser::*;

    pub fn parse_sorted(src: &str) -> Vec<(String, Type)> {
        let (parsed, codemap) = parse_str(src);
        let checked = build(parsed, codemap).unwrap();

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
    use super::test_support::*;

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

        assert_typechecking(src);
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

        assert_typechecking(src);
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

        assert_typechecking(src);
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

    fn assert_typechecking(src: &str) {
        let types = parse_sorted(src);
        insta::assert_yaml_snapshot!(types);
    }
}
