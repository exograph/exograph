use payas_core_model::{
    mapped_arena::MappedArena, serializable_system::InterceptorIndex,
    system_serializer::SystemSerializer,
};
use payas_core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::{Interception, SubsystemBuild, SubsystemBuilder},
    typechecker::typ::Type,
};

use crate::system_builder::ModelWasmSystemWithInterceptors;

pub struct WasmSubsystemBuilder {}

impl SubsystemBuilder for WasmSubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Option<Result<SubsystemBuild, ModelBuildingError>> {
        crate::system_builder::build(&typechecked_system, &base_system).map(|subsystem| {
            let ModelWasmSystemWithInterceptors {
                underlying: subsystem,
                interceptors,
            } = subsystem?;

            let serialized_subsystem = subsystem
                .serialize()
                .map_err(|e| ModelBuildingError::Serialize(e))?;

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
                id: "wasm".to_string(),
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