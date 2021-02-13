use crate::model::types::*;

use std::sync::Arc;

use super::system::ModelSystemParameterTypes;

pub trait QueryProvider {
    fn queries(&self, system: &mut ModelSystemParameterTypes) -> Vec<Operation>;
}

impl QueryProvider for ModelType {
    fn queries(&self, system: &mut ModelSystemParameterTypes) -> Vec<Operation> {
        match &self.kind {
            ModelTypeKind::Primitive => vec![],
            ModelTypeKind::Composite { model_fields: _ } => {
                vec![by_pk_query(self, system), collection_query(self, system)]
            }
        }
    }
}

impl<T> QueryProvider for Arc<T>
where
    T: QueryProvider,
{
    fn queries(&self, system: &mut ModelSystemParameterTypes) -> Vec<Operation> {
        self.as_ref().queries(system)
    }
}

fn by_pk_query(tpe: &ModelType, system: &mut ModelSystemParameterTypes) -> Operation {
    let operation_name = normalized_name(tpe).to_owned();

    let return_type: OperationReturnType = OperationReturnType {
        model_type: Arc::new(tpe.to_owned()),
        model_type_modifier: ModelTypeModifier::NonNull,
    };

    let id_param = Parameter {
        name: "id".to_string(),
        tpe: system.find_parameter_type("Int").unwrap(), // TODO: Use id parameter's type
        type_modifier: ModelTypeModifier::NonNull,
    };

    Operation {
        name: operation_name.clone(),
        parameters: vec![id_param],
        return_type: return_type,
    }
}

fn collection_query(tpe: &ModelType, system: &mut ModelSystemParameterTypes) -> Operation {
    let operation_name = to_plural(normalized_name(tpe));

    let return_type: OperationReturnType = OperationReturnType {
        model_type: Arc::new(tpe.to_owned()),
        model_type_modifier: ModelTypeModifier::List,
    };

    Operation {
        name: operation_name.clone(),
        parameters: vec![order_by_param(&tpe, "orderBy".to_string(), system)],
        return_type: return_type,
    }
}

fn order_by_param(
    tpe: &ModelType,
    name: String,
    system: &mut ModelSystemParameterTypes,
) -> Parameter {
    Parameter {
        name: name,
        tpe: order_by_param_type(tpe, system),
        type_modifier: ModelTypeModifier::Optional,
    }
}

fn order_by_param_type(
    tpe: &ModelType,
    system: &mut ModelSystemParameterTypes,
) -> Arc<ParameterType> {
    match &tpe.kind {
        ModelTypeKind::Primitive => system.find_parameter_type("Ordering").unwrap(),
        ModelTypeKind::Composite { model_fields } => {
            let parameters = model_fields
                .iter()
                .map(|field| order_by_param(&field.tpe, field.name.to_string(), system))
                .collect();

            let param_type_name = format!("{}OrderBy", tpe.name);
            system.find_parameter_type_or(param_type_name.as_str(), || ParameterType {
                name: param_type_name.clone(),
                kind: ParameterTypeKind::Composite { parameters },
            })
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
