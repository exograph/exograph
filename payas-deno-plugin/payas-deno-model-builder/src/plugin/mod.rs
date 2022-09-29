use payas_core_model::{mapped_arena::MappedArena, system::InterceptorIndex};
use payas_core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::{Interception, SubsystemBuild, SubsystemBuilder},
    typechecker::typ::Type,
};

use crate::system_builder::ModelDenoSystemWithInterceptors;

pub struct DenoSubsystemBuilder {}

impl SubsystemBuilder for DenoSubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Result<SubsystemBuild, ModelBuildingError> {
        let ModelDenoSystemWithInterceptors {
            underlying: subsystem,
            interceptors,
        } = crate::system_builder::build(&typechecked_system, &base_system)?;

        let serialized_subsystem = bincode::serialize(&subsystem).map_err(|e| {
            ModelBuildingError::Generic(format!("Failed to serialize deno subsystem: {}", e))
        })?;

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
    }
}
