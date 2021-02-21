use crate::model::types::*;

use super::system::{ModelSystem, ModelSystemParameterTypes};

pub trait QueryProvider {
    fn queries(
        &self,
        system: &ModelSystem,
        param_types: &mut ModelSystemParameterTypes,
    ) -> Vec<Operation>;
}

impl QueryProvider for ModelType {
    fn queries(
        &self,
        system: &ModelSystem,
        param_types: &mut ModelSystemParameterTypes,
    ) -> Vec<Operation> {
        match &self.kind {
            ModelTypeKind::Primitive => vec![],
            ModelTypeKind::Composite { .. } => {
                vec![
                    by_pk_query(self, system, param_types),
                    collection_query(self, system, param_types),
                ]
            }
        }
    }
}

fn by_pk_query(
    tpe: &ModelType,
    _system: &ModelSystem,
    _param_types: &mut ModelSystemParameterTypes,
) -> Operation {
    let operation_name = normalized_name(tpe).to_owned();

    let return_type: OperationReturnType = OperationReturnType {
        type_name: tpe.name.clone(),
        type_modifier: ModelTypeModifier::NonNull,
    };

    let id_param = Parameter {
        name: "id".to_string(),       // TODO: Use the pk column
        type_name: "Int".to_string(), // TODO: Use id parameter's type
        type_modifier: ModelTypeModifier::NonNull,
        role: ParameterRole::Predicate,
    };

    Operation {
        name: operation_name,
        parameters: vec![id_param],
        return_type: return_type,
    }
}

fn collection_query(
    tpe: &ModelType,
    system: &ModelSystem,
    param_types: &mut ModelSystemParameterTypes,
) -> Operation {
    let operation_name = to_plural(normalized_name(tpe));

    let return_type: OperationReturnType = OperationReturnType {
        type_name: tpe.name.clone(),
        type_modifier: ModelTypeModifier::List,
    };

    Operation {
        name: operation_name.clone(),
        parameters: vec![order_by_param(
            &tpe.name,
            "orderBy".to_string(),
            system,
            param_types,
        )],
        return_type: return_type,
    }
}

fn order_by_param(
    type_name: &str,
    name: String,
    system: &ModelSystem,
    param_types: &mut ModelSystemParameterTypes,
) -> Parameter {
    Parameter {
        name: name,
        type_name: order_by_param_type(type_name, system, param_types),
        type_modifier: ModelTypeModifier::Optional,
        role: ParameterRole::OrderBy,
    }
}

fn order_by_param_type(
    type_name: &str,
    system: &ModelSystem,
    param_types: &mut ModelSystemParameterTypes,
) -> String {
    let tpe = system.find_type(&type_name);

    match &tpe.as_ref().unwrap().kind {
        ModelTypeKind::Primitive => "Ordering".to_string(),
        ModelTypeKind::Composite { model_fields, .. } => {
            let parameters = model_fields
                .iter()
                .map(|field| {
                    order_by_param(
                        &field.type_name,
                        field.name.to_string(),
                        system,
                        param_types,
                    )
                })
                .collect();

            let param_type_name = format!("{}OrderBy", &type_name);
            param_types.find_parameter_type_or(param_type_name.as_str(), || ParameterType {
                name: param_type_name.clone(),
                kind: ParameterTypeKind::Composite { parameters },
            });
            param_type_name
        }
    }
}

fn normalized_name(tpe: &ModelType) -> String {
    // Concert -> concert i.e. lowercase the first letter
    let mut ret = tpe.name.to_owned();
    if let Some(r) = ret.get_mut(0..1) {
        r.make_ascii_lowercase();
    }
    ret
}

// TODO: Bring in a proper pluralize implementation
fn to_plural(input: String) -> String {
    format!("{}s", input)
}
