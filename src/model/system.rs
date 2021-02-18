use std::collections::HashMap;

use super::query::*;
use super::types::*;

pub struct ModelSystem {
    pub types: Vec<ModelType>,
    pub queries: Vec<Operation>,
    pub parameter_types: ModelSystemParameterTypes,
}

pub struct ModelSystemParameterTypes {
    primitive_parameter_type_map: HashMap<String, ParameterType>,
    other_parameter_type_map: HashMap<String, ParameterType>,
}

const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list

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
        //self.find_type("Int").unwrap()
        "Int".to_string()
    }

    pub fn string_type(&self) -> String {
        "String".to_string()
    }
}

impl ModelSystemParameterTypes {
    pub fn new() -> Self {
        let primitive_parameter_type_map: HashMap<String, ParameterType> =
            PRIMITIVE_TYPE_NAMES
                .iter()
                .map(|tname| {
                    (
                        tname.to_string(),
                        ParameterType {
                            name: tname.to_string(),
                            kind: ParameterTypeKind::Primitive,
                        },
                    )
                })
                .collect();

        let mut other_parameter_type_map = HashMap::new();
        let ordering_parameter_type = ParameterType {
            name: "Ordering".to_string(),
            kind: ParameterTypeKind::Enum {
                values: vec!["ASC".to_string(), "DESC".to_string()],
            },
        };

        other_parameter_type_map.insert(
            ordering_parameter_type.name.clone(),
            ordering_parameter_type,
        );

        Self {
            primitive_parameter_type_map,
            other_parameter_type_map,
        }
    }

    pub fn non_primitive_parameter_types(&self) -> Vec<&ParameterType> {
        self.other_parameter_type_map.values().collect()
    }

    pub fn find_parameter_type(&self, name: &str) -> Option<&ParameterType> {
        self.primitive_parameter_type_map
            .get(name)
            .or_else(|| self.other_parameter_type_map.get(name))
    }

    pub fn add_parameter_type(&mut self, tpe: ParameterType) {
        self.other_parameter_type_map
            .insert(tpe.name.clone(), tpe);
    }

    pub fn find_parameter_type_or<F>(&mut self, name: &str, default: F) -> &ParameterType
    where
        F: FnOnce() -> ParameterType,
    {
        let entry = self.other_parameter_type_map.entry(name.to_string());
        entry.or_insert(default())
    }
}
