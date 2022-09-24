// use async_graphql_parser::types::{BaseType, Type};
// use async_graphql_value::Name;
// use payas_model::model::{
//     argument::{ArgumentParameter, ArgumentParameterType},
//     limit_offset::{LimitParameter, LimitParameterType, OffsetParameter, OffsetParameterType},
//     operation::{CreateDataParameter, CreateDataParameterTypeWithModifier, UpdateDataParameter},
//     order::{OrderByParameter, OrderByParameterType, OrderByParameterTypeWithModifier},
//     predicate::{PredicateParameter, PredicateParameterType, PredicateParameterTypeWithModifier},
//     system::ModelSystem,
//     GqlType, GqlTypeModifier,
// };

// use super::{
//     definition::GqlFieldDefinition,
//     definition::{GqlFieldTypeDefinition, GqlFieldTypeDefinitionNode, GqlTypeDefinition},
// };

// impl GqlFieldDefinition for PredicateParameter {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
//         &self.typ
//     }

//     fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![] // Input types don't have arguments
//     }
// }

// impl GqlFieldTypeDefinition for PredicateParameterTypeWithModifier {
//     fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
//         &model.predicate_types[self.type_id].name
//     }

//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         let tpe = &model.predicate_types[self.type_id];
//         if self.type_modifier == GqlTypeModifier::NonNull {
//             GqlFieldTypeDefinitionNode::Leaf(tpe)
//         } else {
//             GqlFieldTypeDefinitionNode::NonLeaf(tpe, &self.type_modifier)
//         }
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &self.type_modifier
//     }
// }

// impl GqlFieldTypeDefinition for PredicateParameterType {
//     fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
//         &self.name
//     }

//     fn inner<'a>(&'a self, _model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         GqlFieldTypeDefinitionNode::Leaf(self)
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &GqlTypeModifier::NonNull
//     }
// }

// impl GqlFieldDefinition for OrderByParameter {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
//         &self.typ
//     }

//     fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![] // Input types don't have arguments
//     }
// }

// impl GqlFieldTypeDefinition for OrderByParameterTypeWithModifier {
//     fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
//         &model.order_by_types[self.type_id].name
//     }

//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         let tpe = &model.order_by_types[self.type_id];
//         if self.type_modifier == GqlTypeModifier::NonNull {
//             GqlFieldTypeDefinitionNode::Leaf(tpe)
//         } else {
//             GqlFieldTypeDefinitionNode::NonLeaf(tpe, &self.type_modifier)
//         }
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &self.type_modifier
//     }
// }

// impl GqlFieldTypeDefinition for OrderByParameterType {
//     fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
//         &self.name
//     }

//     fn inner<'a>(&'a self, _model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         GqlFieldTypeDefinitionNode::Leaf(self)
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &GqlTypeModifier::NonNull
//     }
// }

// impl GqlFieldDefinition for LimitParameter {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
//         &self.typ
//     }

//     fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![] // Input types don't have arguments
//     }
// }

// impl GqlFieldTypeDefinition for LimitParameterType {
//     fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
//         &self.type_name
//     }

//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         GqlFieldTypeDefinitionNode::NonLeaf(
//             &model.database_types[self.type_id],
//             &self.type_modifier,
//         )
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &self.type_modifier
//     }
// }

// impl GqlFieldTypeDefinition for GqlType {
//     fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
//         &self.name
//     }

//     fn inner<'a>(&'a self, _model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         GqlFieldTypeDefinitionNode::Leaf(self)
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &GqlTypeModifier::NonNull
//     }
// }

// impl GqlFieldDefinition for OffsetParameter {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
//         &self.typ
//     }

//     fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![] // Input types don't have arguments
//     }
// }

// impl GqlFieldTypeDefinition for OffsetParameterType {
//     fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
//         &self.type_name
//     }

//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         GqlFieldTypeDefinitionNode::NonLeaf(
//             &model.database_types[self.type_id],
//             &self.type_modifier,
//         )
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &self.type_modifier
//     }
// }

// impl GqlFieldDefinition for CreateDataParameter {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
//         &self.typ
//     }

//     fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![] // Input types don't have arguments
//     }
// }

// impl GqlFieldTypeDefinition for CreateDataParameterTypeWithModifier {
//     fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
//         &self.type_name
//     }

//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         let tpe = &model.mutation_types[self.type_id];
//         if self.array_input {
//             GqlFieldTypeDefinitionNode::NonLeaf(tpe, &GqlTypeModifier::List)
//         } else {
//             GqlFieldTypeDefinitionNode::Leaf(tpe)
//         }
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         if self.array_input {
//             &GqlTypeModifier::List
//         } else {
//             &GqlTypeModifier::NonNull
//         }
//     }
// }

// impl GqlFieldDefinition for UpdateDataParameter {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn field_type<'a>(&self, model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
//         &model.mutation_types[self.type_id]
//     }

//     fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![] // Input types don't have arguments
//     }
// }

// impl GqlFieldDefinition for ArgumentParameter {
//     fn name(&self) -> &str {
//         &self.name
//     }

//     fn field_type<'a>(&'a self, _model: &'a ModelSystem) -> &'a dyn GqlFieldTypeDefinition {
//         &self.typ
//     }

//     fn arguments<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![] // Input types don't have arguments
//     }
// }

// impl GqlFieldTypeDefinition for ArgumentParameterType {
//     fn name<'a>(&'a self, _model: &'a ModelSystem) -> &'a str {
//         &self.name
//     }

//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         let tpe = self.type_id.as_ref().map(|t| &model.service_types[*t]);

//         match tpe {
//             Some(tpe) => {
//                 if self.type_modifier == GqlTypeModifier::NonNull {
//                     GqlFieldTypeDefinitionNode::Leaf(tpe)
//                 } else {
//                     GqlFieldTypeDefinitionNode::NonLeaf(tpe, &self.type_modifier)
//                 }
//             }
//             None => {
//                 let tpe = &model
//                     .primitive_types
//                     .iter()
//                     .chain(model.service_types.iter())
//                     .find(|t| t.1.name == self.name)
//                     .unwrap()
//                     .1;
//                 GqlFieldTypeDefinitionNode::Leaf(*tpe)
//             }
//         }
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         &self.type_modifier
//     }
// }

// impl GqlFieldTypeDefinition for Type {
//     fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
//         match &self.base {
//             BaseType::Named(name) => name.as_str(),
//             BaseType::List(underlying) => underlying.name(model),
//         }
//     }

//     fn inner<'a>(&'a self, model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         if self.nullable {
//             GqlFieldTypeDefinitionNode::NonLeaf(&self.base, &GqlTypeModifier::Optional)
//         } else {
//             self.base.inner(model)
//         }
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         if self.nullable {
//             &GqlTypeModifier::Optional
//         } else {
//             self.base.modifier()
//         }
//     }
// }

// impl GqlFieldTypeDefinition for BaseType {
//     fn name<'a>(&'a self, model: &'a ModelSystem) -> &'a str {
//         match self {
//             BaseType::Named(name) => name.as_str(),
//             BaseType::List(underlying) => underlying.name(model),
//         }
//     }

//     fn inner<'a>(&'a self, _model: &'a ModelSystem) -> GqlFieldTypeDefinitionNode<'a> {
//         match self {
//             BaseType::Named(name) => GqlFieldTypeDefinitionNode::Leaf(name),
//             BaseType::List(underlying) => {
//                 GqlFieldTypeDefinitionNode::NonLeaf(underlying.as_ref(), &GqlTypeModifier::List)
//             }
//         }
//     }

//     fn modifier(&self) -> &GqlTypeModifier {
//         match self {
//             BaseType::Named(_) => &GqlTypeModifier::NonNull,
//             BaseType::List(_) => &GqlTypeModifier::List,
//         }
//     }
// }

// impl GqlTypeDefinition for Name {
//     fn name(&self) -> &str {
//         self.as_str()
//     }

//     fn fields<'a>(&'a self, _model: &'a ModelSystem) -> Vec<&'a dyn GqlFieldDefinition> {
//         vec![]
//     }
// }
