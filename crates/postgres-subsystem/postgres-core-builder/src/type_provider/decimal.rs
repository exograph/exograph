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
use exo_sql::PhysicalColumnType;
use postgres_core_model::aggregate::ScalarAggregateFieldKind;
use serde::{Deserialize, Serialize};

use super::PrimitiveTypeProvider;
use crate::resolved_type::{ResolvedField, ResolvedTypeHint, SerializableTypeHint};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecimalTypeHint {
    pub precision: Option<usize>,
    pub scale: Option<usize>,
}

impl ResolvedTypeHint for DecimalTypeHint {
    fn hint_type_name(&self) -> &'static str {
        "Decimal"
    }

    fn serialize_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl PrimitiveTypeProvider for primitive_type::DecimalType {
    fn determine_column_type(&self, field: &ResolvedField) -> PhysicalColumnType {
        match &field.type_hint {
            Some(hint) => {
                let hint_ref = hint.0.as_ref() as &dyn std::any::Any;

                if let Some(decimal_hint) = hint_ref.downcast_ref::<DecimalTypeHint>() {
                    // cannot have scale and no precision specified
                    if decimal_hint.precision.is_none() {
                        assert!(decimal_hint.scale.is_none())
                    }

                    PhysicalColumnType::Numeric {
                        precision: decimal_hint.precision,
                        scale: decimal_hint.scale,
                    }
                } else {
                    PhysicalColumnType::Numeric {
                        precision: None,
                        scale: None,
                    }
                }
            }
            None => PhysicalColumnType::Numeric {
                precision: None,
                scale: None,
            },
        }
    }

    fn compute_type_hint(
        &self,
        field: &AstField<Typed>,
        errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint> {
        let precision_hint = field
            .annotations
            .get("precision")
            .map(|p| p.as_single().as_int() as usize);

        let scale_hint = field
            .annotations
            .get("scale")
            .map(|p| p.as_single().as_int() as usize);

        if scale_hint.is_some() && precision_hint.is_none() {
            errors.push(Diagnostic {
                level: Level::Error,
                message: "@scale is not allowed without specifying @precision".to_string(),
                code: Some("C000".to_string()),
                spans: vec![SpanLabel {
                    span: field.span,
                    style: SpanStyle::Primary,
                    label: None,
                }],
            });
        }

        Some(SerializableTypeHint(Box::new(DecimalTypeHint {
            precision: precision_hint,
            scale: scale_hint,
        })))
    }

    fn deserialize_type_hint(
        &self,
        data: serde_json::Value,
    ) -> Result<Box<dyn ResolvedTypeHint>, String> {
        let hint: DecimalTypeHint = serde_json::from_value(data)
            .map_err(|e| format!("Failed to deserialize DecimalTypeHint: {}", e))?;
        Ok(Box::new(hint))
    }

    fn applicable_hint_annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![
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
                "scale",
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
        Some(vec!["eq", "neq", "lt", "lte", "gt", "gte"])
    }

    fn supported_aggregates(
        &self,
    ) -> Vec<(
        ScalarAggregateFieldKind,
        Option<&'static dyn PrimitiveBaseType>,
    )> {
        use ScalarAggregateFieldKind::*;
        vec![(Min, None), (Max, None), (Sum, None), (Avg, None)]
    }
}
