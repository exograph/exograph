use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

use crate::ast::ast_types::{FieldSelection, Identifier};

use super::{Scope, Type, Typecheck};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TypedFieldSelection {
    Single(Identifier, Type),
    Select(Box<TypedFieldSelection>, Identifier, Type),
}

impl TypedFieldSelection {
    pub fn typ(&self) -> &Type {
        match &self {
            TypedFieldSelection::Single(_, typ) => typ,
            TypedFieldSelection::Select(_, _, typ) => typ,
        }
    }
}

impl Typecheck<TypedFieldSelection> for FieldSelection {
    fn shallow(&self) -> TypedFieldSelection {
        match &self {
            FieldSelection::Single(v) => TypedFieldSelection::Single(v.clone(), Type::Defer),
            FieldSelection::Select(selection, i) => {
                TypedFieldSelection::Select(Box::new(selection.shallow()), i.clone(), Type::Defer)
            }
        }
    }

    fn pass(&self, typ: &mut TypedFieldSelection, env: &MappedArena<Type>, scope: &Scope) -> bool {
        match &self {
            FieldSelection::Single(Identifier(i)) => {
                if let TypedFieldSelection::Single(_, Type::Defer) = typ {
                    if i.as_str() == "self" {
                        if let Some(enclosing) = &scope.enclosing_model {
                            *typ = TypedFieldSelection::Single(
                                Identifier(i.clone()),
                                Type::Reference(enclosing.clone()),
                            );
                            true
                        } else {
                            *typ = TypedFieldSelection::Single(
                                Identifier(i.clone()),
                                Type::Error("Cannot use self outside a model".to_string()),
                            );
                            false
                        }
                    } else {
                        *typ = TypedFieldSelection::Single(
                            Identifier(i.clone()),
                            Type::Error(format!("Reference to unknown value: {}", i)),
                        );
                        false
                    }
                } else {
                    false
                }
            }
            FieldSelection::Select(selection, i) => {
                if let TypedFieldSelection::Select(prefix, _, typ) = typ {
                    let in_updated = selection.pass(prefix, env, scope);
                    let out_updated = if typ.is_incomplete() {
                        if let Type::Composite(c) = prefix.typ().deref(env) {
                            if let Some(field) = c.fields.iter().find(|f| f.name == i.0) {
                                if !field.typ.is_incomplete() {
                                    assert!(*typ != field.typ.clone());
                                    *typ = field.typ.clone();
                                    true
                                } else {
                                    *typ = Type::Error(format!(
                                        "Cannot read field {} in model {:?} with incomplete type",
                                        i.0,
                                        prefix.typ()
                                    ));
                                    false
                                }
                            } else {
                                *typ = Type::Error(format!("No such field: {}", i.0));
                                false
                            }
                        } else {
                            *typ = Type::Error(format!(
                                "Cannot read field {} from a non-composite type {:?}",
                                i.0,
                                prefix.typ()
                            ));
                            false
                        }
                    } else {
                        false
                    };

                    in_updated || out_updated
                } else {
                    panic!()
                }
            }
        }
    }
}
