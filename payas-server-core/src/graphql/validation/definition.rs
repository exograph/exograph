use std::fmt::Debug;

use payas_model::model::{system::ModelSystem, GqlTypeModifier};

// Normalizing trait for various container types (input and output types) such as:
// - A list of queries/mutations. Here name would be the root query/mutation name and fields will be individual queries/mutations
// - User-defined models such as `Concert`. Here the name would be the user-specified type name and field would be the fields of the model
// - Clay-defined parameters such as `ConcertPredicate`. Much like the above, but type is defined internally by Clay.
pub trait GqlTypeDefinition: Debug {
    fn name(&self) -> &str;

    fn fields<'a>(&'a self, model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition>;
}

/// Normalized representation of a field.
/// A field has a name, a type, and a list of arguments.
///
/// The difference from a typical "struct" kind of type definition is that the fields take a list of arguments (so they are more like a method definition).
///
/// GraphQL type system has a few intricacies:
/// - An output field has a (possibly empty) list of arguments (for example, `venues` inside `Concert`). So more like a method.
/// - An input field doesn't have that argument list. So more like a field in a struct.
///
/// Example: For a field such as: `concerts(where: ConcertWhereInput): [Concert]` (as a field in the `Query` root type),
/// the normalized representation has:
/// - name: "concerts"
/// - type: [Concert]
/// - arguments: `{name: "where", type: [ConcertWhereInput]}`
pub trait GqlFieldDefinition: Debug {
    fn name(&self) -> &str;

    fn field_type<'a>(&'a self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition;

    fn arguments<'a>(&'a self, model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition>;
}

/// Normalized field type with a modifier such as optional, list, or non-null.
pub trait GqlFieldTypeDefinition: Debug {
    fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str;

    // Unwrap one level of modifier. For example, if the type is `[Concert]`, this will return `Concert`.
    // If there is no modifier, this will return `None`.
    fn inner<'a>(&'a self, model: &'a ModelSystem) -> Option<&'a dyn GqlFieldTypeDefinition>;

    fn modifier(&self) -> &GqlTypeModifier;
}
