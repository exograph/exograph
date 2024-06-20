// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::{
    async_trait::async_trait,
    core_model_builder::{
        builder::system_builder::BaseModelSystem,
        error::ModelBuildingError,
        plugin::SubsystemBuild,
        typechecker::{
            annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParamSpec},
            typ::TypecheckedSystem,
        },
    },
    interface::SubsystemBuilder,
    system_serializer::SystemSerializer,
};

pub struct PostgresSubsystemBuilder {}

#[async_trait]
impl SubsystemBuilder for PostgresSubsystemBuilder {
    fn id(&self) -> &'static str {
        "postgres"
    }

    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![
            (
                "postgres",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Module],
                    no_params: true,
                    single_params: false,
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
                "maxLength",
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
                "plural",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Type],
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
                "bits16",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "bits32",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "bits64",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "singlePrecision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "size", // vector size
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "distanceFunction", // vector distance function
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: false,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "doublePrecision",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: false,
                    mapped_params: None,
                },
            ),
            (
                "table",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Type],
                    no_params: false,
                    single_params: true,
                    mapped_params: Some(&[
                        MappedAnnotationParamSpec {
                            name: "name",
                            optional: true,
                        },
                        MappedAnnotationParamSpec {
                            name: "schema",
                            optional: true,
                        },
                    ]),
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
            (
                "index",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: true,
                    mapped_params: None,
                },
            ),
            (
                "update",
                AnnotationSpec {
                    targets: &[AnnotationTarget::Field],
                    no_params: true,
                    single_params: true,
                    mapped_params: None,
                },
            ),
        ]
    }

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
    ) -> Result<Option<SubsystemBuild>, ModelBuildingError> {
        let subsystem = crate::system_builder::build(typechecked_system, base_system)?;
        let Some(subsystem) = subsystem else {
            return Ok(None);
        };

        let serialized_subsystem = subsystem
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        Ok(Some(SubsystemBuild {
            id: "postgres".to_string(),
            serialized_subsystem,
            query_names: {
                let pk_query_names = subsystem.pk_queries.iter().map(|(_, q)| q.name.clone());

                let collection_query_names = subsystem
                    .collection_queries
                    .iter()
                    .map(|(_, q)| q.name.clone());

                let aggregate_query_names = subsystem
                    .aggregate_queries
                    .iter()
                    .map(|(_, q)| q.name.clone());

                pk_query_names
                    .chain(collection_query_names)
                    .chain(aggregate_query_names)
                    .collect()
            },
            mutation_names: subsystem
                .mutations
                .iter()
                .map(|(_, q)| q.name.clone())
                .collect(),
            interceptions: vec![],
        }))
    }
}
