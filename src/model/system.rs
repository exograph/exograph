use std::collections::HashMap;

use super::operation::*;
use super::order::*;
use super::predicate::*;
use super::query::*;
use super::types::*;

#[derive(Debug, Clone)]
pub struct ModelSystem {
    pub types: Vec<ModelType>,
    pub queries: Vec<Query>,
    pub parameter_types: ModelSystemParameterTypes,
}

#[derive(Debug, Clone)]
pub struct ModelSystemParameterTypes {
    pub predicate_parameter_type_map: HashMap<String, PredicateParameterType>,
    pub order_by_parameter_type_map: HashMap<String, OrderByParameterType>,
}

const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list
const OPERATORS: [&str; 3] = ["eq", "lt", "gt"]; // TODO: Expand

impl ModelSystem {
    pub fn new() -> Self {
        let primitive_types: Vec<ModelType> = PRIMITIVE_TYPE_NAMES
            .iter()
            .map(|tname| ModelType {
                name: tname.to_string(),
                kind: ModelTypeKind::Primitive,
            })
            .collect();

        ModelSystem {
            types: primitive_types,
            parameter_types: ModelSystemParameterTypes::new(),
            queries: vec![],
        }
    }

    pub fn find_type(&self, name: &str) -> Option<&ModelType> {
        self.types.iter().find(|tpe| tpe.name == name)
    }

    pub fn add_type(&mut self, tpe: ModelType) {
        self.types.push(tpe);
    }

    // Build operations
    // Typically called after all types have been added to the system
    pub fn build(&mut self) {
        let mut parameter_types = ModelSystemParameterTypes::new();
        self.queries = self
            .types
            .iter()
            .flat_map(|tpe| tpe.queries(self, &mut parameter_types))
            .collect();

        self.parameter_types = parameter_types;
    }

    // Helper commonly needed functions
    pub fn int_type(&self) -> String {
        "Int".to_string()
    }

    pub fn string_type(&self) -> String {
        "String".to_string()
    }
}

impl ModelSystemParameterTypes {
    pub fn new() -> Self {
        let predicate_parameter_type_map: HashMap<String, PredicateParameterType> =
            PRIMITIVE_TYPE_NAMES
                .iter()
                .flat_map(|tname| {
                    let filter_type = Self::create_scalar_filter_param_ype(tname.to_string());
                    vec![
                        (
                            tname.to_string(),
                            PredicateParameterType {
                                name: tname.to_string(),
                                kind: PredicateParameterTypeKind::Primitive,
                            },
                        ),
                        (filter_type.name.clone(), filter_type),
                    ]
                })
                .collect();

        let mut order_by_parameter_type_map = HashMap::new();
        let ordering_parameter_type = OrderByParameterType {
            name: "Ordering".to_string(),
            kind: OrderByParameterTypeKind::Enum {
                values: vec!["ASC".to_string(), "DESC".to_string()],
            },
        };

        order_by_parameter_type_map.insert(
            ordering_parameter_type.name.clone(),
            ordering_parameter_type,
        );

        Self {
            predicate_parameter_type_map,
            order_by_parameter_type_map,
        }
    }

    pub fn find_order_by_parameter_type(&self, name: &str) -> Option<&OrderByParameterType> {
        self.order_by_parameter_type_map.get(name)
    }

    pub fn find_order_by_parameter_type_or<F>(
        &mut self,
        name: &str,
        default: F,
    ) -> &OrderByParameterType
    where
        F: FnOnce() -> OrderByParameterType,
    {
        let entry = self.order_by_parameter_type_map.entry(name.to_string());
        entry.or_insert(default())
    }

    pub fn find_predicate_parameter_type(&self, name: &str) -> Option<&PredicateParameterType> {
        self.predicate_parameter_type_map.get(name)
    }

    pub fn find_predicate_parameter_type_or<F>(
        &mut self,
        name: &str,
        default: F,
    ) -> &PredicateParameterType
    where
        F: FnOnce() -> PredicateParameterType,
    {
        let entry = self.predicate_parameter_type_map.entry(name.to_string());
        entry.or_insert(default())
    }

    fn create_scalar_filter_param_ype(scalar_type: String) -> PredicateParameterType {
        let type_name = format!("{}Filter", scalar_type);

        let parameters: Vec<_> = OPERATORS
            .iter()
            .map(|operator| PredicateParameter {
                name: operator.to_string(),
                type_name: scalar_type.clone(),
                type_modifier: ModelTypeModifier::Optional,
            })
            .collect();

        PredicateParameterType {
            name: type_name,
            kind: PredicateParameterTypeKind::Composite {
                parameters,
                primitive_filter: true,
            },
        }
    }
}
