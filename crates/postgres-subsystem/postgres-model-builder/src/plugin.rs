use core_model::mapped_arena::MappedArena;
use core_model_builder::{
    builder::system_builder::BaseModelSystem, error::ModelBuildingError, plugin::SubsystemBuild,
    typechecker::typ::Type,
};
use core_plugin_interface::interface::SubsystemBuilder;
use core_plugin_shared::system_serializer::SystemSerializer;

pub struct PostgresSubsystemBuilder {}
core_plugin_interface::export_subsystem_builder!(PostgresSubsystemBuilder {});

impl SubsystemBuilder for PostgresSubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Option<Result<SubsystemBuild, ModelBuildingError>> {
        let subsystem = crate::system_builder::build(typechecked_system, base_system);

        subsystem.map(|subsystem| {
            let subsystem = subsystem?;

            let serialized_subsystem = subsystem
                .serialize()
                .map_err(ModelBuildingError::Serialize)?;

            Ok(SubsystemBuild {
                id: "postgres".to_string(),
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
                interceptions: vec![],
            })
        })
    }
}
