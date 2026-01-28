use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::{
    primitive_type::{self, PrimitiveBaseType},
    types::{IntConstraints, TypeValidation, TypeValidationProvider},
};
use core_model_builder::{
    ast::ast_types::AstField,
    builder::resolved_builder::AnnotationMapHelper,
    typechecker::{
        Typed,
        annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParamSpec},
    },
};
use exo_sql::{IntBits, IntColumnType, PhysicalColumnType};
use postgres_core_model::aggregate::ScalarAggregateFieldKind;
use serde::{Deserialize, Serialize};

use super::PrimitiveTypeProvider;
use crate::resolved_type::{ResolvedField, ResolvedTypeHint, SerializableTypeHint};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntTypeHint {
    pub bits: Option<usize>,
    pub range: Option<(i64, i64)>,
}

impl ResolvedTypeHint for IntTypeHint {
    fn hint_type_name(&self) -> &'static str {
        "Int"
    }

    fn serialize_data(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl TypeValidationProvider for IntTypeHint {
    fn get_type_validation(&self) -> Option<TypeValidation> {
        self.range
            .as_ref()
            .map(|r| TypeValidation::Int(IntConstraints::from_range(r.0, r.1)))
    }
}

impl PrimitiveTypeProvider for primitive_type::IntType {
    fn determine_column_type(&self, field: &ResolvedField) -> Box<dyn PhysicalColumnType> {
        match &field.type_hint {
            Some(hint) => {
                let hint_ref = hint.0.as_ref() as &dyn std::any::Any;

                if let Some(int_hint) = hint_ref.downcast_ref::<IntTypeHint>() {
                    // determine the proper sized type to use
                    if let Some(bits) = int_hint.bits {
                        Box::new(IntColumnType {
                            bits: match bits {
                                16 => IntBits::_16,
                                32 => IntBits::_32,
                                64 => IntBits::_64,
                                _ => panic!("Invalid bits"),
                            },
                        })
                    } else if let Some(range) = &int_hint.range {
                        let is_superset = |bound_min: i64, bound_max: i64| {
                            let range_min = range.0;
                            let range_max = range.1;
                            assert!(range_min <= range_max);
                            assert!(bound_min <= bound_max);

                            // is this bound a superset of the provided range?
                            (bound_min <= range_min && bound_min <= range_max)
                                && (bound_max >= range_max && bound_max >= range_min)
                        };

                        // determine which SQL type is appropriate for this range
                        if is_superset(i16::MIN.into(), i16::MAX.into()) {
                            Box::new(IntColumnType { bits: IntBits::_16 })
                        } else if is_superset(i32::MIN.into(), i32::MAX.into()) {
                            Box::new(IntColumnType { bits: IntBits::_32 })
                        } else if is_superset(i64::MIN, i64::MAX) {
                            Box::new(IntColumnType { bits: IntBits::_64 })
                        } else {
                            // TODO: numeric type
                            panic!("Requested range is too big")
                        }
                    } else {
                        // no specific hints provided, go with default
                        Box::new(IntColumnType { bits: IntBits::_32 })
                    }
                } else {
                    // no relevant hints provided, go with default
                    Box::new(IntColumnType { bits: IntBits::_32 })
                }
            }
            None => {
                // no hints provided, go with default
                Box::new(IntColumnType { bits: IntBits::_32 })
            }
        }
    }

    fn compute_type_hint(
        &self,
        field: &AstField<Typed>,
        errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint> {
        let range_hint = field.annotations.get("range").map(|params| {
            (
                params.as_map().get("min").unwrap().as_int(),
                params.as_map().get("max").unwrap().as_int(),
            )
        });

        let is_bits16 = field.annotations.contains("bits16");
        let is_bits32 = field.annotations.contains("bits32");
        let is_bits64 = field.annotations.contains("bits64");

        let bits_hint = match (is_bits16, is_bits32, is_bits64) {
            (true, false, false) => Some(16),
            (false, true, false) => Some(32),
            (false, false, true) => Some(64),
            (false, false, false) => None,
            _ => {
                errors.push(Diagnostic {
                    level: Level::Error,
                    message: "Cannot have more than one of @bits16, @bits32, @bits64".to_string(),
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
            Some(SerializableTypeHint(Box::new(IntTypeHint {
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
        let hint: IntTypeHint = serde_json::from_value(data)
            .map_err(|e| format!("Failed to deserialize IntTypeHint: {}", e))?;
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
                "bits16",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "bits32",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "bits64",
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
            (Sum, None),
            (Avg, Some(&primitive_type::FloatType)),
        ]
    }
}
