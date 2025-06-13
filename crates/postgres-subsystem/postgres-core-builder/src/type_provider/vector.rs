use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::primitive_type::{self, PrimitiveBaseType};
use core_model_builder::{
    ast::ast_types::AstField,
    builder::resolved_builder::AnnotationMapHelper,
    typechecker::{
        Typed,
        annotation::{AnnotationSpec, AnnotationTarget},
    },
};
use exo_sql::{DEFAULT_VECTOR_SIZE, PhysicalColumnType, VectorColumnType, VectorDistanceFunction};
use postgres_core_model::aggregate::ScalarAggregateFieldKind;
use serde::{Deserialize, Serialize};

use super::PrimitiveTypeProvider;
use crate::resolved_type::{ResolvedField, ResolvedTypeHint, SerializableTypeHint};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorTypeHint {
    pub size: Option<usize>,
    pub distance_function: Option<VectorDistanceFunction>,
}

impl ResolvedTypeHint for VectorTypeHint {
    fn hint_type_name(&self) -> &'static str {
        "Vector"
    }

    fn serialize_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl PrimitiveTypeProvider for primitive_type::VectorType {
    fn determine_column_type(&self, field: &ResolvedField) -> Box<dyn PhysicalColumnType> {
        match &field.type_hint {
            Some(hint) => {
                let hint_ref = hint.0.as_ref() as &dyn std::any::Any;

                if let Some(vector_hint) = hint_ref.downcast_ref::<VectorTypeHint>() {
                    Box::new(VectorColumnType {
                        size: vector_hint.size.unwrap_or(DEFAULT_VECTOR_SIZE),
                    })
                } else {
                    Box::new(VectorColumnType {
                        size: DEFAULT_VECTOR_SIZE,
                    })
                }
            }
            None => Box::new(VectorColumnType {
                size: DEFAULT_VECTOR_SIZE,
            }),
        }
    }

    fn compute_type_hint(
        &self,
        field: &AstField<Typed>,
        errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint> {
        let size = field
            .annotations
            .get("size")
            .map(|p| p.as_single().as_int() as usize);

        let distance_function = field.annotations.get("distanceFunction").and_then(|p| {
            match VectorDistanceFunction::from_model_string(p.as_single().as_string().as_str()) {
                Ok(distance_function) => Some(distance_function),
                Err(e) => {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: e.to_string(),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: field.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                    None
                }
            }
        });

        Some(SerializableTypeHint(Box::new(VectorTypeHint {
            size,
            distance_function,
        })))
    }

    fn deserialize_type_hint(
        &self,
        data: serde_json::Value,
    ) -> Result<Box<dyn ResolvedTypeHint>, String> {
        let hint: VectorTypeHint = serde_json::from_value(data)
            .map_err(|e| format!("Failed to deserialize VectorTypeHint: {}", e))?;
        Ok(Box::new(hint))
    }

    fn applicable_hint_annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![
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
                "distanceFunction",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
        ]
    }

    fn supported_operators(&self) -> Option<Vec<&'static str>> {
        Some(vec!["similar", "eq", "neq"])
    }

    fn supported_aggregates(
        &self,
    ) -> Vec<(
        ScalarAggregateFieldKind,
        Option<&'static dyn PrimitiveBaseType>,
    )> {
        use ScalarAggregateFieldKind::*;
        vec![(Avg, None)]
    }
}
