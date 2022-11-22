use std::vec;

use crate::system_builder::ModelWasmSystemWithInterceptors;
use core_plugin_interface::{
    core_model::mapped_arena::MappedArena,
    core_model_builder::{
        builder::system_builder::BaseModelSystem,
        error::ModelBuildingError,
        plugin::{Interception, SubsystemBuild},
        typechecker::{
            annotation::{AnnotationSpec, AnnotationTarget},
            typ::Type,
        },
    },
    interception::InterceptorIndex,
    interface::SubsystemBuilder,
    system_serializer::SystemSerializer,
};
pub struct WasmSubsystemBuilder {}
core_plugin_interface::export_subsystem_builder!(WasmSubsystemBuilder {});

impl SubsystemBuilder for WasmSubsystemBuilder {
    fn id(&self) -> &'static str {
        "wasm"
    }

    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![(
            "wasm",
            AnnotationSpec {
                targets: &[AnnotationTarget::Service],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        )]
    }

    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Option<Result<SubsystemBuild, ModelBuildingError>> {
        crate::system_builder::build(typechecked_system, base_system).map(|subsystem| {
            let ModelWasmSystemWithInterceptors {
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
