use id_arena::Id;

use crate::model::{
    operation::{MutationDataParameter, OperationReturnType},
    type_builder,
    types::ModelTypeModifier,
};

use super::{
    ast::ast_types::{AstType, AstTypeKind},
    operation::CreateMutation,
    system_context::SystemContextBuilding,
    types::ModelType,
};

pub fn build_shallow(ast_types: &[AstType], building: &mut SystemContextBuilding) {
    for ast_type in ast_types.iter() {
        if let AstTypeKind::Composite { .. } = &ast_type.kind {
            let model_type_id = building.types.get_id(&ast_type.name).unwrap();
            let create_mutation = build_create_mutation(model_type_id, ast_type, building);

            building
                .create_mutations
                .add(&create_mutation.name.to_owned(), create_mutation);
        }
    }
}

fn build_create_mutation(
    model_type_id: Id<ModelType>,
    ast_type: &AstType,
    building: &SystemContextBuilding,
) -> CreateMutation {
    let data_param_type_name = type_builder::input_type_name(&ast_type.name);
    let data_param_type_id = building
        .mutation_types
        .get_id(&data_param_type_name)
        .unwrap();

    CreateMutation {
        name: format!("create{}", &ast_type.name),
        data_param: MutationDataParameter {
            name: "data".to_string(),
            type_name: data_param_type_name,
            type_id: data_param_type_id,
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.name.to_string(),
            type_modifier: ModelTypeModifier::Optional,
        },
    }
}
