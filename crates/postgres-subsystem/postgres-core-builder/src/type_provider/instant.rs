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
pub struct DateTimeTypeHint {
    pub precision: usize,
}

impl ResolvedTypeHint for DateTimeTypeHint {
    fn hint_type_name(&self) -> &'static str {
        "DateTime"
    }

    fn serialize_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl PrimitiveTypeProvider for primitive_type::InstantType {
    fn determine_column_type(&self, field: &ResolvedField) -> PhysicalColumnType {
        match &field.type_hint {
            Some(hint) => {
                let hint_ref = hint.0.as_ref() as &dyn std::any::Any;

                if let Some(datetime_hint) = hint_ref.downcast_ref::<DateTimeTypeHint>() {
                    PhysicalColumnType::Timestamp {
                        precision: Some(datetime_hint.precision),
                        timezone: true,
                    }
                } else {
                    PhysicalColumnType::Timestamp {
                        precision: None,
                        timezone: true,
                    }
                }
            }
            None => PhysicalColumnType::Timestamp {
                precision: None,
                timezone: true,
            },
        }
    }

    fn compute_type_hint(
        &self,
        field: &AstField<Typed>,
        _errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint> {
        field.annotations.get("precision").map(|p| {
            SerializableTypeHint(Box::new(DateTimeTypeHint {
                precision: p.as_single().as_int() as usize,
            }))
        })
    }

    fn deserialize_type_hint(
        &self,
        data: serde_json::Value,
    ) -> Result<Box<dyn ResolvedTypeHint>, String> {
        let hint: DateTimeTypeHint = serde_json::from_value(data)
            .map_err(|e| format!("Failed to deserialize DateTimeTypeHint: {}", e))?;
        Ok(Box::new(hint))
    }

    fn applicable_hint_annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![(
            "precision",
            AnnotationSpec {
                targets: &[AnnotationTarget::Field],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        )]
    }

    fn supported_operators(&self) -> Option<Vec<&'static str>> {
        Some(vec!["eq", "neq", "lt", "lte", "gt", "gte"])
    }

    fn supported_aggregates(
        &self,
    ) -> Vec<(
        ScalarAggregateFieldKind,
        Option<&'static dyn PrimitiveBaseType>,
    )> {
        vec![]
    }
}
