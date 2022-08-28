use std::fmt::Debug;

use payas_model::model::{system::ModelSystem, GqlTypeModifier};

// Normalizing trait for various container types (input and output types) such as:
// - A list of queries/mutations. Here name would be the root query/mutation name and fields will be individual queries/mutations
// - User-defined models such as `Concert`. Here the name would be the user-specified type name and field would be the fields of the model
// - Clay-defined parameters such as `ConcertPredicate`. Much like the above, but type is defined internally by Clay.
pub trait GqlTypeDefinition: Debug {
    fn name(&self) -> &str;

    fn fields(&self) -> Vec<&dyn GqlFieldDefinition>;
}

/// Normalized representation of a field.
/// A field has a name, a type, and a list of arguments.
///
/// The difference from a typical "struct" kind of type definition is that the fields take a list of arguments.
///
/// GraphQL type system has a few intricacies:
/// - An output field has a list of arguments (for example, `venues` inside `Concert`). May be empty.
/// - An input field doesn't have that argument list
///
/// Example: For a field such as: `concerts(where: ConcertWhereInput): [Concert]` (as a field in the `Query` root type),
/// the normalized representation has:
/// - name: "concerts"
/// - type: [Concert]
/// - arguments: `{name: "where", type: [ConcertWhereInput]}`
pub trait GqlFieldDefinition: Debug {
    fn name(&self) -> &str;

    fn ty<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition;

    fn arguments<'a>(&'a self, model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition>;
}

/// Normalized field type with a modifier such as optional, list, or non-null.
pub trait GqlFieldTypeDefinition: Debug {
    fn name(&self) -> &str;

    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition>;

    fn leaf<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlTypeDefinition;

    fn modifier(&self) -> &GqlTypeModifier;
}

// /// Normalized representation of a parameter.
// pub trait GqlParameterDefinition: Debug {
//     fn name(&self) -> &str;

//     fn ty<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlParameterTypeDefinition;
// }

// pub trait GqlParameterTypeDefinition: Debug {
//     fn name(&self) -> &str;

//     fn underlying<'a>(
//         &'a self,
//         model: &'a ModelSystem,
//     ) -> Option<&'a dyn GqlParameterTypeDefinition>;

//     fn modifier(&self) -> &GqlTypeModifier;
// }
