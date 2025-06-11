use codemap_diagnostic::Diagnostic;
use core_model::primitive_type::{self, PrimitiveBaseType};
use core_model_builder::{
    ast::ast_types::AstField,
    builder::resolved_builder::AnnotationMapHelper,
    typechecker::{
        Typed,
        annotation::{AnnotationSpec, AnnotationTarget},
    },
};
use exo_sql::PhysicalColumnType;
use postgres_core_model::aggregate::ScalarAggregateFieldKind;
use serde::{Deserialize, Serialize};

use super::PrimitiveTypeProvider;
use crate::resolved_type::{ResolvedField, ResolvedTypeHint, SerializableTypeHint};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringTypeHint {
    pub max_length: usize,
}

impl ResolvedTypeHint for StringTypeHint {
    fn hint_type_name(&self) -> &'static str {
        "String"
    }

    fn serialize_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl PrimitiveTypeProvider for primitive_type::StringType {
    fn determine_column_type(&self, field: &ResolvedField) -> PhysicalColumnType {
        match &field.type_hint {
            Some(hint) => {
                let hint_ref = hint.0.as_ref() as &dyn std::any::Any;

                if let Some(string_hint) = hint_ref.downcast_ref::<StringTypeHint>() {
                    // length hint provided, use it
                    PhysicalColumnType::String {
                        max_length: Some(string_hint.max_length),
                    }
                } else {
                    PhysicalColumnType::String { max_length: None }
                }
            }
            None => PhysicalColumnType::String { max_length: None },
        }
    }

    fn compute_type_hint(
        &self,
        field: &AstField<Typed>,
        _errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint> {
        let max_length_annotation = field
            .annotations
            .get("maxLength")
            .map(|p| p.as_single().as_int() as usize);

        max_length_annotation
            .map(|max_length| SerializableTypeHint(Box::new(StringTypeHint { max_length })))
    }

    fn deserialize_type_hint(
        &self,
        data: serde_json::Value,
    ) -> Result<Box<dyn ResolvedTypeHint>, String> {
        let hint: StringTypeHint = serde_json::from_value(data)
            .map_err(|e| format!("Failed to deserialize StringTypeHint: {}", e))?;
        Ok(Box::new(hint))
    }

    fn applicable_hint_annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![(
            "maxLength",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        )]
    }

    fn supported_operators(&self) -> Option<Vec<&'static str>> {
        Some(vec![
            "eq",
            "neq",
            "lt",
            "lte",
            "gt",
            "gte",
            "like",
            "ilike",
            "startsWith",
            "endsWith",
        ])
    }

    fn supported_aggregates(
        &self,
    ) -> Vec<(
        ScalarAggregateFieldKind,
        Option<&'static dyn PrimitiveBaseType>,
    )> {
        use ScalarAggregateFieldKind::*;
        vec![(Min, None), (Max, None)]
    }
}
