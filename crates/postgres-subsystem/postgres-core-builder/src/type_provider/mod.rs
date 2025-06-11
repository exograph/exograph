use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use codemap_diagnostic::{Diagnostic, Level, SpanLabel, SpanStyle};
use core_model::primitive_type::{self, PrimitiveBaseType};
use core_model_builder::{
    ast::ast_types::AstField,
    typechecker::{Typed, annotation::AnnotationSpec},
};
use exo_sql::PhysicalColumnType;
use postgres_core_model::aggregate::ScalarAggregateFieldKind;

use crate::resolved_type::{ResolvedField, ResolvedTypeHint, SerializableTypeHint};

// Import all type provider modules
mod blob;
mod boolean;
mod decimal;
mod float;
pub mod instant;
mod int;
mod json;
mod local_date;
mod local_date_time;
mod local_time;
mod string;
mod uuid;
mod vector;

// Re-export type hints
pub use decimal::DecimalTypeHint;
pub use float::FloatTypeHint;
pub use instant::DateTimeTypeHint;
pub use int::IntTypeHint;
pub use string::StringTypeHint;
pub use vector::VectorTypeHint;

/// Provide Postgres-specific functionality for primitive types
/// Handles physical column type determination, type hints, annotations, and deserialization
pub trait PrimitiveTypeProvider: Send + Sync + PrimitiveBaseType {
    /// Determines the physical column type for a field with this primitive type
    fn determine_column_type(&self, field: &ResolvedField) -> PhysicalColumnType;

    /// Computes the type hint for a field, validating that only supported hint annotations are used.
    fn compute_type_hint(
        &self,
        field: &AstField<Typed>,
        errors: &mut Vec<Diagnostic>,
    ) -> Option<SerializableTypeHint>;

    /// Deserialize JSON data into a type hint for this provider
    /// Returns an error for primitive types that don't support type hints.
    fn deserialize_type_hint(
        &self,
        _data: serde_json::Value,
    ) -> Result<Box<dyn ResolvedTypeHint>, String> {
        Err(format!("Type {} does not support type hints", self.name()))
    }

    /// Get applicable hint annotations for this type
    /// Returns an empty vector for types that don't support type hints.
    fn applicable_hint_annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![]
    }

    /// Get supported operators for this primitive type
    /// Returns None if the type supports no operators (implicit equality only)
    /// Returns Some(operators) if the type supports specific operators
    fn supported_operators(&self) -> Option<Vec<&'static str>>;

    /// Get supported aggregate functions for this primitive type
    /// Returns a vector of (aggregate_kind, optional_return_type) tuples
    /// The return type is Some when the aggregate function returns a different type than the input
    /// (e.g., avg of Int returns Float). The "count" aggregate is always supported and doesn't need to be listed here.
    fn supported_aggregates(
        &self,
    ) -> Vec<(
        ScalarAggregateFieldKind,
        Option<&'static dyn PrimitiveBaseType>,
    )> {
        vec![]
    }
}

/// Unified registry mapping primitive type names to their providers
/// Handles all primitive type functionality: column types, type hints, annotations, deserialization
pub static PRIMITIVE_TYPE_PROVIDER_REGISTRY: LazyLock<
    HashMap<&'static str, &'static dyn PrimitiveTypeProvider>,
> = LazyLock::new(|| {
    let all_primitive_type_providers: &[&dyn PrimitiveTypeProvider] = &[
        &primitive_type::IntType,
        &primitive_type::FloatType,
        &primitive_type::DecimalType,
        &primitive_type::StringType,
        &primitive_type::InstantType,
        &primitive_type::VectorType,
        &primitive_type::BooleanType,
        &primitive_type::LocalDateType,
        &primitive_type::LocalTimeType,
        &primitive_type::LocalDateTimeType,
        &primitive_type::JsonType,
        &primitive_type::BlobType,
        &primitive_type::UuidType,
    ];
    all_primitive_type_providers
        .iter()
        .map(|provider| (provider.name(), *provider))
        .collect()
});

static ALL_HINT_ANNOTATION_NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    collect_all_hint_annotations()
        .iter()
        .map(|(name, _)| *name)
        .collect()
});

/// Validates that only supported hint annotations are used for a given field and type provider
pub fn validate_hint_annotations(
    field: &AstField<Typed>,
    provider: &dyn PrimitiveTypeProvider,
    errors: &mut Vec<Diagnostic>,
) {
    let field_annotations = field.annotations.annotations.keys();
    let supported_annotations: Vec<&'static str> = provider
        .applicable_hint_annotations()
        .iter()
        .map(|(name, _)| *name)
        .collect();
    let supported_hint_annotations_set: std::collections::HashSet<&str> =
        supported_annotations.iter().copied().collect();

    let unsupported_hint_annotations = field_annotations
        .filter(|annotation| {
            // Only validate against hint annotations that this type supports
            // Skip annotations that are not hint annotations (they'll be validated elsewhere)
            ALL_HINT_ANNOTATION_NAMES.contains(annotation.as_str())
                && !supported_hint_annotations_set.contains(annotation.as_str())
        })
        .collect::<Vec<_>>();

    if !unsupported_hint_annotations.is_empty() {
        errors.push(Diagnostic {
            level: Level::Error,
            message: format!(
                "Annotation @{} is not supported for type {}",
                unsupported_hint_annotations
                    .iter()
                    .map(|a| a.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                provider.name()
            ),
            code: Some("C000".to_string()),
            spans: vec![SpanLabel {
                span: field.span,
                style: SpanStyle::Primary,
                label: None,
            }],
        });
    }
}

/// Collects all hint annotations from all registered type hint providers
pub fn collect_all_hint_annotations() -> Vec<(&'static str, AnnotationSpec)> {
    PRIMITIVE_TYPE_PROVIDER_REGISTRY
        .iter()
        .flat_map(|(_, provider)| provider.applicable_hint_annotations())
        .collect()
}
