use payas_model::model::system::ModelSystem;

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

// fn underlying_type(typ: &GqlFieldType) -> &str {
//     match &typ {
//         GqlFieldType::Optional(underlying) | GqlFieldType::List(underlying) => {
//             underlying_type(underlying)
//         }
//         GqlFieldType::Reference { type_id, type_name } => &type_name,
//     }
// }

fn find_type<'a>(model: &'a ModelSystem, name: &str) -> Option<&'a dyn GqlTypeDefinition> {
    model
        .types
        .iter()
        .find(|t| t.1.name.as_str() == name)
        .map(|t| t.1 as &dyn GqlTypeDefinition)
}

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
        .mutation_types
        .iter()
        .find(|t| t.1.name.as_str() == name)
        .map(|t| t.1 as &dyn GqlTypeDefinition)
    {
        return Some(typ);
    }

    if let Some(typ) = model
        .argument_types
        .iter()
        .find(|t| t.1.name.as_str() == name)
        .map(|t| t.1 as &dyn GqlTypeDefinition)
    {
        return Some(typ);
    }

    None
}
