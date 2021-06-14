use payas_model::model::mapped_arena::MappedArena;

use crate::ast::ast_types::{AstModel, AstModelKind};

use super::{typ::CompositeTypeKind, CompositeType, Scope, Type, Typecheck};

impl Typecheck<Type> for AstModel {
    fn shallow(&self) -> Type {
        Type::Composite(CompositeType {
            name: self.name.clone(),
            kind: if self.kind == AstModelKind::Persistent {
                CompositeTypeKind::Persistent
            } else {
                CompositeTypeKind::Context
            },
            fields: self.fields.iter().map(|f| f.shallow()).collect(),
            annotations: self.annotations.iter().map(|a| a.shallow()).collect(),
        })
    }

    fn pass(&self, typ: &mut Type, env: &MappedArena<Type>, _scope: &Scope, errors: &mut Vec< codemap_diagnostic::Diagnostic>) -> bool {
        if let Type::Composite(c) = typ {
            let model_scope = Scope {
                enclosing_model: Some(self.name.clone()),
            };
            let fields_changed = self
                .fields
                .iter()
                .zip(c.fields.iter_mut())
                .map(|(f, tf)| f.pass(tf, env, &model_scope, errors))
                .filter(|v| *v)
                .count()
                > 0;

            let annot_changed = self
                .annotations
                .iter()
                .zip(c.annotations.iter_mut())
                .map(|(ast_annot, typed_annot)| ast_annot.pass(typed_annot, env, &model_scope, errors))
                .filter(|v| *v)
                .count()
                > 0;

            fields_changed || annot_changed
        } else {
            panic!()
        }
    }
}
