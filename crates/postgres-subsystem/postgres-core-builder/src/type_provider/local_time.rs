use codemap_diagnostic::Diagnostic;
use core_model::primitive_type::{self, PrimitiveBaseType};
use core_model_builder::{ast::ast_types::AstField, typechecker::Typed};
use exo_sql::{PhysicalColumnType, TimeColumnType};
use postgres_core_model::aggregate::ScalarAggregateFieldKind;

use super::{PrimitiveTypeProvider, instant::DateTimeTypeHint};
use crate::resolved_type::{ResolvedField, SerializableTypeHint};

impl PrimitiveTypeProvider for primitive_type::LocalTimeType {
    fn determine_column_type(&self, field: &ResolvedField) -> Box<dyn PhysicalColumnType> {
        match &field.type_hint {
            Some(hint) => {
                let hint_ref = hint.0.as_ref() as &dyn std::any::Any;

                if let Some(datetime_hint) = hint_ref.downcast_ref::<DateTimeTypeHint>() {
                    Box::new(TimeColumnType {
                        precision: Some(datetime_hint.precision),
                    })
                } else {
                    Box::new(TimeColumnType { precision: None })
                }
            }
            None => Box::new(TimeColumnType { precision: None }),
        }
    }

    fn compute_type_hint(
        &self,
        _field: &AstField<Typed>,
        _errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint> {
        None
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
