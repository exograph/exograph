use payas_core_model::type_normalization::TypeModifier;
use payas_database_model::types::DatabaseTypeModifier;
use payas_model::model::system::ModelSystem;
use payas_service_model::types::ServiceTypeModifier;

use self::definition::GqlTypeDefinition;

/// Validate the query payload.
///
/// Take a user submitted query along with the operation name and variables (from the request payload)
/// and transform the query into a validated form (in the process, validate the query).
pub mod operation;

pub mod document_validator;

mod arguments_validator;
mod definition;
mod gql_field_definition;
mod gql_parameter_definition;
mod gql_type_definition;
mod operation_validator;
mod selection_set_validator;
pub mod validation_error;

// impl From<DatabaseTypeModifier> for TypeModifier {
//     fn from(modifier: DatabaseTypeModifier) -> Self {
//         match modifier {
//             DatabaseTypeModifier::List => TypeModifier::List,
//             DatabaseTypeModifier::NonNull => TypeModifier::NonNull,
//             DatabaseTypeModifier::Optional => TypeModifier::Optional,
//         }
//     }
// }

// impl From<ServiceTypeModifier> for TypeModifier {
//     fn from(modifier: ServiceTypeModifier) -> Self {
//         match modifier {
//             ServiceTypeModifier::List => TypeModifier::List,
//             ServiceTypeModifier::NonNull => TypeModifier::NonNull,
//             ServiceTypeModifier::Optional => TypeModifier::Optional,
//         }
//     }
// }

fn find_arg_type<'a>(model: &'a ModelSystem, name: &str) -> Option<&'a dyn GqlTypeDefinition> {
    if let Some(typ) = model
        .predicate_types
        .iter()
        .find(|t| t.1.name.as_str() == name)
        .map(|t| t.1 as &dyn GqlTypeDefinition)
    {
        return Some(typ);
    }

    if let Some(typ) = model
        .order_by_types
        .iter()
        .find(|t| t.1.name.as_str() == name)
        .map(|t| t.1 as &dyn GqlTypeDefinition)
    {
        return Some(typ);
    }

    if let Some(typ) = model
        .mutation_types
        .iter()
        .find(|t| t.1.name.as_str() == name)
        .map(|t| t.1 as &dyn GqlTypeDefinition)
    {
        return Some(typ);
    }

    if let Some(typ) = model
        .service_types
        .iter()
        .find(|t| t.1.name.as_str() == name)
        .map(|t| t.1 as &dyn GqlTypeDefinition)
    {
        return Some(typ);
    }

    None
}
