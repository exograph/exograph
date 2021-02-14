use std::{collections::HashMap, sync::Arc};

use super::query::*;
use super::types::*;

pub struct ModelSystem {
    pub types: Vec<Arc<ModelType>>,
    pub queries: Vec<Operation>,
    pub parameter_types: ModelSystemParameterTypes,
}

pub struct ModelSystemParameterTypes {
    primitive_parameter_type_map: HashMap<String, Arc<ParameterType>>,
    other_parameter_type_map: HashMap<String, Arc<ParameterType>>,
}

const PRIMITIVE_TYPE_NAMES: [&str; 2] = ["Int", "String"]; // TODO: Expand the list

impl ModelSystem {
    pub fn new() -> Self {
        let primitive_types: Vec<Arc<ModelType>> = PRIMITIVE_TYPE_NAMES
            .iter()
            .map(|tname| ModelType {
                name: tname.to_string(),
                kind: ModelTypeKind::Primitive,
            })
            .map(Arc::new)
            .collect();

        ModelSystem {
            types: primitive_types,
            parameter_types: ModelSystemParameterTypes::new(),
            queries: vec![],
        }
    }

    pub fn find_type(&self, name: &str) -> Option<Arc<ModelType>> {
        self.types.iter().find(|tpe| tpe.name == name).cloned()
    }

    pub fn add_type(&mut self, tpe: ModelType) {
        self.types.push(Arc::new(tpe));
    }

    // Build operations
    // Typically called after all types have been added to the system
    pub fn build(&mut self) {
        let parameter_types = &mut self.parameter_types;
        self.queries = self
            .types
            .iter()
            .flat_map(|tpe| tpe.queries(parameter_types))
            .collect();
    }

    // Helper commonly needed functions
    pub fn int_type(&self) -> Arc<ModelType> {
        self.find_type("Int").unwrap()
    }

    pub fn string_type(&self) -> Arc<ModelType> {
        self.find_type("String").unwrap()
    }
}

impl ModelSystemParameterTypes {
    pub fn new() -> Self {
        let primitive_parameter_type_map: HashMap<String, Arc<ParameterType>> =
            PRIMITIVE_TYPE_NAMES
                .iter()
                .map(|tname| {
                    (
                        tname.to_string(),
                        Arc::new(ParameterType {
                            name: tname.to_string(),
                            kind: ParameterTypeKind::Primitive,
                        }),
                    )
                })
                .collect();

        let mut other_parameter_type_map = HashMap::new();
        let ordering_parameter_type = Arc::new(ParameterType {
            name: "Ordering".to_string(),
            kind: ParameterTypeKind::Enum {
                values: vec!["ASC".to_string(), "DESC".to_string()],
            },
        });

        other_parameter_type_map.insert(
            ordering_parameter_type.name.clone(),
            ordering_parameter_type,
        );

        Self {
            primitive_parameter_type_map,
            other_parameter_type_map,
        }
    }

    pub fn non_primitive_parameter_types(&self) -> Vec<&Arc<ParameterType>> {
        self.other_parameter_type_map.values().collect()
    }

    pub fn find_parameter_type(&self, name: &str) -> Option<Arc<ParameterType>> {
        self.primitive_parameter_type_map
            .get(name)
            .or_else(|| self.other_parameter_type_map.get(name))
            .cloned()
    }

    pub fn add_parameter_type(&mut self, tpe: ParameterType) {
        self.other_parameter_type_map
            .insert(tpe.name.clone(), Arc::new(tpe));
    }

    pub fn find_parameter_type_or<F>(&mut self, name: &str, default: F) -> Arc<ParameterType>
    where
        F: FnOnce() -> ParameterType,
    {
        let entry = self.other_parameter_type_map.entry(name.to_string());
        entry.or_insert(Arc::new(default())).to_owned()
    }
}
