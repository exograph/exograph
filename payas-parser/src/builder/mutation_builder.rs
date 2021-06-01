use id_arena::Id;
use payas_model::model::{operation::Mutation, types::ModelType};

use crate::builder::{query_builder, type_builder};

use payas_model::model::{
    operation::{MutationDataParameter, MutationKind, OperationReturnType},
    types::ModelTypeModifier,
};

use super::{system_builder::SystemContextBuilding, typechecking::Type};

pub fn build(ast_types: &[Type], building: &mut SystemContextBuilding) {
    for ast_type in ast_types.iter() {
        let model_type_id = building.types.get_id(&ast_type.composite_name()).unwrap();
        let create_mutation = build_create_mutation(model_type_id, ast_type, building);

        building
            .mutations
            .add(&create_mutation.name.to_owned(), create_mutation);

        for mutation in build_delete_mutations(model_type_id, ast_type, building)
            .into_iter()
            .chain(build_update_mutations(model_type_id, ast_type, building).into_iter())
        {
            building.mutations.add(&mutation.name.to_owned(), mutation);
        }
    }
}

fn build_create_mutation(
    model_type_id: Id<ModelType>,
    ast_type: &Type,
    building: &SystemContextBuilding,
) -> Mutation {
    let data_param_type_name = type_builder::input_creation_type_name(&ast_type.composite_name());
    let data_param_type_id = building
        .mutation_types
        .get_id(&data_param_type_name)
        .unwrap();

    Mutation {
        name: format!("create{}", &ast_type.composite_name()),
        kind: MutationKind::Create(MutationDataParameter {
            name: "data".to_string(),
            type_name: data_param_type_name,
            type_id: data_param_type_id,
        }),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.composite_name(),
            type_modifier: ModelTypeModifier::Optional,
        },
    }
}

fn build_delete_mutations(
    model_type_id: Id<ModelType>,
    ast_type: &Type,
    building: &SystemContextBuilding,
) -> Vec<Mutation> {
    let model_type = &building.types[model_type_id];
    let by_pk_delete = Mutation {
        name: format!("delete{}", &ast_type.composite_name()),
        kind: MutationKind::Delete(query_builder::pk_predicate_param(model_type, building)),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.composite_name(),
            type_modifier: ModelTypeModifier::Optional,
        },
    };

    let by_predicate_delete = Mutation {
        name: format!("delete{}s", &ast_type.composite_name()),
        kind: MutationKind::Delete(query_builder::collection_predicate_param(
            model_type, building,
        )),
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.composite_name(),
            type_modifier: ModelTypeModifier::List,
        },
    };

    vec![by_pk_delete, by_predicate_delete]
}

fn build_update_mutations(
    model_type_id: Id<ModelType>,
    ast_type: &Type,
    building: &SystemContextBuilding,
) -> Vec<Mutation> {
    let model_type = &building.types[model_type_id];

    let data_param_type_name = type_builder::input_update_type_name(&ast_type.composite_name());
    let data_param_type_id = building
        .mutation_types
        .get_id(&data_param_type_name)
        .unwrap();

    let by_pk_update = Mutation {
        name: format!("update{}", &ast_type.composite_name()),
        kind: MutationKind::Update {
            data_param: MutationDataParameter {
                name: "data".to_string(),
                type_name: data_param_type_name.clone(),
                type_id: data_param_type_id,
            },
            predicate_param: query_builder::pk_predicate_param(model_type, building),
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.composite_name(),
            type_modifier: ModelTypeModifier::Optional,
        },
    };

    let by_predicate_update = Mutation {
        name: format!("update{}s", &ast_type.composite_name()),
        kind: MutationKind::Update {
            data_param: MutationDataParameter {
                name: "data".to_string(),
                type_name: data_param_type_name,
                type_id: data_param_type_id,
            },
            predicate_param: query_builder::collection_predicate_param(model_type, building),
        },
        return_type: OperationReturnType {
            type_id: model_type_id,
            type_name: ast_type.composite_name(),
            type_modifier: ModelTypeModifier::List,
        },
    };

    vec![by_pk_update, by_predicate_update]
}
