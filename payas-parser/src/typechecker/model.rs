use std::collections::HashMap;

use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstModel, AstModelKind};

use super::annotation::AnnotationSpec;
use super::{typ::CompositeTypeKind, AnnotationMap, CompositeType, Scope, Type, Typecheck};

impl Typecheck<Type> for AstModel {
    fn shallow(&self) -> Type {
        let annotation_map = AnnotationMap::new(&self.annotations);

        Type::Composite(CompositeType {
            name: self.name.clone(),
            kind: if self.kind == AstModelKind::Persistent {
                CompositeTypeKind::Persistent
            } else {
                CompositeTypeKind::Context
            },
            fields: self.fields.iter().map(|f| f.shallow()).collect(),
            annotation_map,
        })
    }

    fn pass(
        &self,
        typ: &mut Type,
        type_env: &MappedArena<Type>,
        annotation_env: &HashMap<String, AnnotationSpec>,
        _scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        if let Type::Composite(c) = typ {
            let model_scope = Scope {
                enclosing_model: Some(self.name.clone()),
            };
            let fields_changed = self
                .fields
                .iter()
                .zip(c.fields.iter_mut())
                .map(|(f, tf)| f.pass(tf, type_env, annotation_env, &model_scope, errors))
                .filter(|v| *v)
                .count()
                > 0;

            let annot_changed = c.annotation_map.pass(
                &self.annotations,
                type_env,
                annotation_env,
                &model_scope,
                errors,
            );

            fields_changed || annot_changed
        } else {
            panic!()
        }
    }
}
