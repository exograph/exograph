// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::interceptor_weaver::{self, OperationKind};
use core_model_builder::error::ModelBuildingError;
use core_model_builder::typechecker::typ::TypecheckedSystem;
use core_plugin_interface::interface::SubsystemBuilder;
use core_plugin_shared::serializable_system::SerializableSubsystem;
use core_plugin_shared::serializable_system::SerializableSystem;
use core_plugin_shared::system_serializer::SystemSerializer;
use core_plugin_shared::trusted_documents::TrustedDocuments;

/// Build a [ModelSystem] given an [AstSystem].
///
/// First, it type checks the input [AstSystem] to produce typechecked types.
/// Next, it resolves the typechecked types. Resolving a type entails consuming annotations and finalizing information such as table and column names.
/// Finally, it builds the type through a series of builders.
///
/// Each builder implements the following pattern:
/// - build_shallow: Build relevant shallow types.
///   Each shallow type in marked as primitive and thus holds just the name and notes if it is an input type.
/// - build_expanded: Fully expand the previously created shallow type as well as any other dependent objects (such as Query and Mutation)
///
/// This two pass method allows dealing with cycles.
/// In the first shallow pass, each builder iterates over resolved types and create a placeholder type.
/// In the second expand pass, each builder again iterates over resolved types and expand each type
/// (this is done in place, so references created from elsewhere remain valid). Since all model
/// types have been created in the first pass, the expansion pass can refer to other types (which may still be
/// shallow if hasn't had its chance in the iteration, but will expand when its turn comes in).
pub async fn build(
    subsystem_builders: &[Box<dyn SubsystemBuilder + Send + Sync>],
    typechecked_system: TypecheckedSystem,
    trusted_documents: TrustedDocuments,
) -> Result<Vec<u8>, ModelBuildingError> {
    let base_system = core_model_builder::builder::system_builder::build(&typechecked_system)?;

    let mut subsystem_interceptions = vec![];
    let mut query_names = vec![];
    let mut mutation_names = vec![];

    // We must enumerate() over the result of running each builder, since that will filter out any
    // subsystem that don't need serialization (empty subsystems). This will ensure that we assign
    // the correct subsystem indices (which will be eventually used to dispatch interceptors to the
    // correct subsystem)
    let subsystems: Vec<SerializableSubsystem> = futures::future::join_all(
        subsystem_builders
            .iter()
            .map(|builder| builder.build(&typechecked_system, &base_system)),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<_>, ModelBuildingError>>()?
    .into_iter()
    .flatten()
    .enumerate()
    .map(|(subsystem_index, build_info)| {
        subsystem_interceptions.push((subsystem_index, build_info.interceptions));
        query_names.extend(build_info.query_names);
        mutation_names.extend(build_info.mutation_names);

        SerializableSubsystem {
            id: build_info.id,
            subsystem_index,
            serialized_subsystem: build_info.serialized_subsystem,
        }
    })
    .collect();

    let query_interception_map =
        interceptor_weaver::weave(&query_names, &subsystem_interceptions, OperationKind::Query);

    let mutation_interception_map = interceptor_weaver::weave(
        &mutation_names,
        &subsystem_interceptions,
        OperationKind::Mutation,
    );

    let system = SerializableSystem {
        subsystems,
        query_interception_map,
        mutation_interception_map,
        trusted_documents,
    };

    system.serialize().map_err(ModelBuildingError::Serialize)
}
