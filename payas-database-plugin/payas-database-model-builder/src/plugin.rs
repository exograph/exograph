use payas_core_model::mapped_arena::MappedArena;
use payas_core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::{SubsystemBuild, SubsystemBuilder},
    typechecker::typ::Type,
};

pub struct DatabaseSubsystemBuilder {}

impl SubsystemBuilder for DatabaseSubsystemBuilder {
    fn build(
        &self,
        typechecked_system: &MappedArena<Type>,
        base_system: &BaseModelSystem,
    ) -> Result<SubsystemBuild, ModelBuildingError> {
        let subsystem = crate::system_builder::build(&typechecked_system, &base_system)?;

        let serialized_subsystem = bincode::serialize(&subsystem).map_err(|e| {
            ModelBuildingError::Generic(format!("Failed to serialize database subsystem: {}", e))
        })?;

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
    }
}
