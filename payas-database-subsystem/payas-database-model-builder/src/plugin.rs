use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::{SubsystemBuild, SubsystemBuilder},
    typechecker::typ::Type,
};
use payas_core_plugin::system_serializer::SystemSerializer;

pub struct DatabaseSubsystemBuilder {}

impl SubsystemBuilder for DatabaseSubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Option<Result<SubsystemBuild, ModelBuildingError>> {
        let subsystem = crate::system_builder::build(&typechecked_system, &base_system);

        subsystem.map(|subsystem| {
            let subsystem = subsystem?;

            let serialized_subsystem = subsystem
                .serialize()
                .map_err(|e| ModelBuildingError::Serialize(e))?;

            Ok(SubsystemBuild {
                id: "database".to_string(),
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
