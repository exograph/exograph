use core_model::mapped_arena::MappedArena;
use core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::SubsystemBuild,
    typechecker::{
        annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParamSpec},
        typ::Type,
    },
};
use core_plugin_interface::interface::SubsystemBuilder;
use core_plugin_shared::system_serializer::SystemSerializer;

pub struct PostgresSubsystemBuilder {}
core_plugin_interface::export_subsystem_builder!(PostgresSubsystemBuilder {});

impl SubsystemBuilder for PostgresSubsystemBuilder {
    fn id(&self) -> &'static str {
        "postgres"
    }

    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![
            (
                "postgres",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Service],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "bits",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "column",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "dbtype",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "length",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "pk",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "plural_name",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Model],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "precision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "range",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: false,
                    mapped_params: Some(&[
                        MappedAnnotationParamSpec {
                            name: "min",
                            optional: false,
                        },
                        MappedAnnotationParamSpec {
                            name: "max",
                            optional: false,
                        },
                    ]),
                },
            ),
            (
                "scale",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "size",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "table",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Model],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "unique",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: true,
                    mapped_params: None,
                },
            ),
        ]
    }

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
