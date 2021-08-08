use std::collections::HashMap;

use crate::ast::ast_types::AstAnnotation;

use super::annotation_params::TypedAnnotationParams;
use super::{Scope, Type, Typecheck, TypedAnnotation};
use payas_model::model::mapped_arena::MappedArena;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnnotationMap {
    annotations: HashMap<String, TypedAnnotation>,
}

impl Default for AnnotationMap {
    fn default() -> Self {
        AnnotationMap {
            annotations: HashMap::new(),
        }
    }
}

impl AnnotationMap {
    pub fn add(&mut self, annotation: TypedAnnotation) {
        self.annotations.insert(annotation.name.clone(), annotation);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.annotations.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&TypedAnnotationParams> {
        self.annotations.get(name).map(|a| &a.params)
    }

    pub fn pass(
        &mut self,
        ast_annotations: &[AstAnnotation],
        env: &MappedArena<Type>,
        scope: &Scope,
        errors: &mut Vec<codemap_diagnostic::Diagnostic>,
    ) -> bool {
        // TODO check name exists
        // TODO check params are correct format
        let mut changed = false;
        for (name, annotation) in &mut self.annotations {
            let annot_changed = ast_annotations
                .iter()
                .find(|a| a.name.as_str() == name)
                .unwrap()
                .pass(annotation, env, scope, errors);

            changed |= annot_changed;
        }
        changed
    }
}
