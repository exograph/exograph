use core_model::mapped_arena::MappedArena;
use core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::{Interception, SubsystemBuild, SubsystemBuilder},
    typechecker::typ::Type,
};
use core_plugin::{interception::InterceptorIndex, system_serializer::SystemSerializer};

use crate::system_builder::ModelDenoSystemWithInterceptors;

pub struct DenoSubsystemBuilder {}

impl SubsystemBuilder for DenoSubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Option<Result<SubsystemBuild, ModelBuildingError>> {
        let subsystem = crate::system_builder::build(typechecked_system, base_system);

        subsystem.map(|subsystem| {
            let ModelDenoSystemWithInterceptors {
                underlying: subsystem,
                interceptors,
            } = subsystem?;

            let serialized_subsystem = subsystem
                .serialize()
                .map_err(ModelBuildingError::Serialize)?;

            let interceptions = interceptors
                .into_iter()
                .map(|(expr, index)| {
                    let interceptor = &subsystem.interceptors[index];
                    let kind = interceptor.interceptor_kind.clone();

                    Interception {
                        expr,
                        kind,
                        index: InterceptorIndex(index.to_idx()),
                    }
                })
                .collect();

            Ok(SubsystemBuild {
                id: "deno".to_string(),
                serialized_subsystem,
                query_names: subsystem
                    .queries
                    .iter()
                    .map(|(_, q)| q.name.clone())
                    .collect(),
                mutation_names: subsystem
                    .mutations
                    .iter()
                    .map(|(_, q)| q.name.clone())
                    .collect(),
                interceptions,
            })
        })
    }
}
