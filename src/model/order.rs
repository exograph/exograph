use super::{system::{ModelSystem, ModelSystemParameterTypes}, types::{ModelTypeKind, ModelTypeModifier}};

#[derive(Debug, Clone)]
pub struct OrderByParameter {
    pub name: String,
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
}

#[derive(Debug, Clone)]
pub struct OrderByParameterType {
    pub name: String,
    pub kind: OrderByParameterTypeKind,
}

#[derive(Debug, Clone)]
pub enum OrderByParameterTypeKind {
    Composite { parameters: Vec<OrderByParameter> },
    Enum { values: Vec<String> },
}

impl OrderByParameter {
    pub fn new(
        type_name: &str,
        name: String,
        system: &ModelSystem,
        system_param_types: &mut ModelSystemParameterTypes,
    ) -> OrderByParameter {
        OrderByParameter {
            name: name,
            type_name: Self::order_by_param_type(type_name, system, system_param_types),
            // Specifying ModelTypeModifier::List allows queries such as:
            // order_by: [{name: ASC}, {id: DESC}]
            // Using a List is the only way to maintain ordering within a parameter value
            // (the order within an object is not guaranteed to be maintained (and the graphql-parser use BTreeMap that doesn't maintain so))
            //
            // But this also allows nonsensical queries such as
            // order_by: [{name: ASC, id: DESC}].
            // Here the user intention is the same as the query above, but we cannot honor that intention
            // This seems like an inherent limit of GraphQL types system (perhaps, input union type proposal will help fix this)
            // TODO: When executing, check for the unsupported version (more than one attributes in an array element) and return an error
            type_modifier: ModelTypeModifier::List,
        }
    }
    
    fn order_by_param_type(
        type_name: &str,
        system: &ModelSystem,
        system_param_types: &mut ModelSystemParameterTypes,
    ) -> String {
        let tpe = system.find_type(&type_name);
    
        match &tpe.as_ref().unwrap().kind {
            ModelTypeKind::Primitive => "Ordering".to_string(),
            ModelTypeKind::Composite { model_fields, .. } => {
                let parameters = model_fields
                    .iter()
                    .map(|field| {
                        Self::new(
                            &field.type_name,
                            field.name.to_string(),
                            system,
                            system_param_types,
                        )
                    })
                    .collect();
    
                let param_type_name = format!("{}OrderBy", &type_name);
                system_param_types.find_order_by_parameter_type_or(param_type_name.as_str(), || OrderByParameterType {
                    name: param_type_name.clone(),
                    kind: OrderByParameterTypeKind::Composite { parameters },
                });
                param_type_name
            }
        }
    }
    
}