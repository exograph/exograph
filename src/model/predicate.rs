use super::{system::{ModelSystem, ModelSystemParameterTypes}, types::{ModelTypeKind, ModelTypeModifier}};


#[derive(Debug, Clone)]
pub struct PredicateParameter {
    pub name: String,
    pub type_name: String,
    pub type_modifier: ModelTypeModifier,
}

#[derive(Debug, Clone)]
pub struct PredicateParameterType {
    pub name: String,
    pub kind: PredicateParameterTypeKind,
}

#[derive(Debug, Clone)]
pub enum PredicateParameterTypeKind {
    Primitive,
    //PrimitiveOperator { scalar_type: String, operator: String },
    Composite { parameters: Vec<PredicateParameter> },
}

impl PredicateParameter {
    pub fn new_pk(
        _type_name: &str,
        _system: &ModelSystem,
        _system_param_types: &mut ModelSystemParameterTypes, // Won't need until we support composite PK
    ) -> PredicateParameter {
        // TODO: Find type using type_name and system to get the name and type of the PK parameter
        PredicateParameter {
            name: "id".to_string(),       // TODO: Use the pk column
            type_name: "Int".to_string(), // TODO: Use id parameter's type
            type_modifier: ModelTypeModifier::NonNull,
        }
    }

    pub fn new_collection(
        type_name: &str,
        name: &str,
        system: &ModelSystem,
        system_param_types: &mut ModelSystemParameterTypes,
    ) -> PredicateParameter {
        let param_type = Self::param_type(type_name, system, system_param_types);
        PredicateParameter {
            name: name.to_string(),
            type_name: param_type,
            type_modifier: ModelTypeModifier::Optional,
        }
    }

    fn param_type(
        type_name: &str,
        system: &ModelSystem,
        system_param_types: &mut ModelSystemParameterTypes,
    ) -> String {
        let tpe = system.find_type(&type_name);
    
        match &tpe.as_ref().unwrap().kind {
            ModelTypeKind::Primitive => {
                format!("{}Filter", type_name)
            },
            ModelTypeKind::Composite { model_fields, .. } => {
                let parameters = model_fields
                    .iter()
                    .map(|field| {
                        Self::new_collection(
                            &field.type_name,
                            &field.name,
                            system,
                            system_param_types,
                        )
                    })
                    .collect();
    
                let param_type_name = format!("{}Filter", &type_name);
                system_param_types.find_predicate_parameter_type_or(param_type_name.as_str(), || PredicateParameterType {
                    name: param_type_name.clone(),
                    kind: PredicateParameterTypeKind::Composite { parameters },
                });
                param_type_name
            }
        }
    }    
}
