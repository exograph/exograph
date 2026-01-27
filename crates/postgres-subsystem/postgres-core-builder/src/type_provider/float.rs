use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::{
    primitive_type::{self, PrimitiveBaseType},
    types::{FloatConstraints, TypeValidation, TypeValidationProvider},
};
use core_model_builder::{
    ast::ast_types::AstField,
    builder::resolved_builder::AnnotationMapHelper,
    typechecker::{
        Typed,
        annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParamSpec},
    },
};
use exo_sql::{FloatBits, FloatColumnType, PhysicalColumnType};
use postgres_core_model::aggregate::ScalarAggregateFieldKind;
use serde::{Deserialize, Serialize};

use super::PrimitiveTypeProvider;
use crate::resolved_type::{ResolvedField, ResolvedTypeHint, SerializableTypeHint};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FloatTypeHint {
    pub bits: Option<usize>,
    pub range: Option<(f64, f64)>,
}

impl ResolvedTypeHint for FloatTypeHint {
    fn hint_type_name(&self) -> &'static str {
        "Float"
    }

    fn serialize_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl TypeValidationProvider for FloatTypeHint {
    fn get_type_validation(&self) -> Option<TypeValidation> {
        self.range
            .as_ref()
            .map(|r| TypeValidation::Float(FloatConstraints::from_range(r.0, r.1)))
    }
}

impl PrimitiveTypeProvider for primitive_type::FloatType {
    fn determine_column_type(&self, field: &ResolvedField) -> Box<dyn PhysicalColumnType> {
        match &field.type_hint {
            Some(hint) => {
                let hint_ref = hint.0.as_ref() as &dyn std::any::Any;

                if let Some(float_hint) = hint_ref.downcast_ref::<FloatTypeHint>() {
                    if let Some(bits) = float_hint.bits {
                        if (1..=24).contains(&bits) {
                            Box::new(FloatColumnType {
                                bits: FloatBits::_24,
                            })
                        } else if bits > 24 && bits <= 53 {
                            Box::new(FloatColumnType {
                                bits: FloatBits::_53,
                            })
                        } else {
                            panic!("Invalid bits")
                        }
                    } else {
                        Box::new(FloatColumnType {
                            bits: FloatBits::_53,
                        })
                    }
                } else {
                    Box::new(FloatColumnType {
                        bits: FloatBits::_24,
                    })
                }
            }
            None => Box::new(FloatColumnType {
                bits: FloatBits::_24,
            }),
        }
    }

    fn compute_type_hint(
        &self,
        field: &AstField<Typed>,
        errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint> {
        let mut range_hint = None;
        if let Some(params) = field.annotations.get("range") {
            let min = params
                .as_map()
                .get("min")
                .unwrap()
                .as_string()
                .parse::<f64>();
            let max = params
                .as_map()
                .get("max")
                .unwrap()
                .as_string()
                .parse::<f64>();

            if let (Ok(min_value), Ok(max_value)) = (&min, &max) {
                range_hint = Some((*min_value, *max_value));
            } else {
                if min.is_err() {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: "Cannot parse @range 'min' as f64".to_string(),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: field.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }
                if max.is_err() {
                    errors.push(Diagnostic {
                        level: Level::Error,
                        message: "Cannot parse @range 'max' as f64".to_string(),
                        code: Some("C000".to_string()),
                        spans: vec![SpanLabel {
                            span: field.span,
                            style: SpanStyle::Primary,
                            label: None,
                        }],
                    });
                }
            }
        };

        let is_single_precision = field.annotations.contains("singlePrecision");
        let is_double_precision = field.annotations.contains("doublePrecision");

        let bits_hint = match (is_single_precision, is_double_precision) {
            (true, false) => Some(24),
            (false, true) => Some(53),
            (false, false) => None,
            _ => {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: "Cannot have both @singlePrecision and @doublePrecision".to_string(),
                    code: Some("C000".to_string()),
                    spans: vec![SpanLabel {
                        span: field.span,
                        style: SpanStyle::Primary,
                        label: None,
                    }],
                });
                None
            }
        };

        if bits_hint.is_some() || range_hint.is_some() {
            Some(SerializableTypeHint(Box::new(FloatTypeHint {
                bits: bits_hint,
                range: range_hint,
            })))
        } else {
            None
        }
    }

    fn deserialize_type_hint(
        &self,
        data: serde_json::Value,
    ) -> Result<Box<dyn ResolvedTypeHint>, String> {
        let hint: FloatTypeHint = serde_json::from_value(data)
            .map_err(|e| format!("Failed to deserialize FloatTypeHint: {}", e))?;
        Ok(Box::new(hint))
    }

    fn applicable_hint_annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![
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
                "singlePrecision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "doublePrecision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
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
        vec![
            (Min, None),
            (Max, None),
            (Sum, Some(&primitive_type::FloatType)),
            (Avg, Some(&primitive_type::FloatType)),
        ]
    }
}
