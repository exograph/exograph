use codemap_diagnostic::Diagnostic;
use core_model::primitive_type::{self, PrimitiveBaseType};
use core_model_builder::{ast::ast_types::AstField, typechecker::Typed};
use exo_sql::PhysicalColumnType;
use postgres_core_model::aggregate::ScalarAggregateFieldKind;

use super::PrimitiveTypeProvider;
use crate::resolved_type::{ResolvedField, SerializableTypeHint};

impl PrimitiveTypeProvider for primitive_type::LocalDateType {
    fn determine_column_type(&self, _field: &ResolvedField) -> PhysicalColumnType {
        PhysicalColumnType::Date
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
