// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_model::function_defn::FunctionDefinition;
use core_model::primitive_type::PrimitiveType;
use core_model::types::FieldType;
use core_model::{context_type::ContextType, mapped_arena::MappedArena};

use crate::error::ModelBuildingError;
use crate::typechecker::typ::TypecheckedSystem;

use super::{context_builder, resolved_builder, type_builder};

#[derive(Debug, Default)]
pub struct SystemContextBuilding {
    pub primitive_types: MappedArena<PrimitiveType>,
    pub contexts: MappedArena<ContextType>,
}

#[derive(Debug)]
pub struct BaseModelSystem {
    pub primitive_types: MappedArena<PrimitiveType>,
    pub contexts: MappedArena<ContextType>,
    pub function_definitions: MappedArena<FunctionDefinition>,
}

pub fn build(
    typechecked_system: &TypecheckedSystem,
) -> Result<BaseModelSystem, ModelBuildingError> {
    let mut building = SystemContextBuilding {
        primitive_types: MappedArena::default(),
        contexts: MappedArena::default(),
    };

    type_builder::build_primitives(&typechecked_system.types, &mut building);

    let resolved = resolved_builder::build(&typechecked_system.types)?;

    context_builder::build(&resolved.contexts, &mut building);

    let mut function_definitions = MappedArena::default();

    vec![FunctionDefinition {
        name: "contains".to_string(),
        return_type: FieldType::Plain(PrimitiveType::Boolean),
    }]
    .into_iter()
    .for_each(|defn| {
        function_definitions.add(&defn.name.clone(), defn);
    });

    Ok(BaseModelSystem {
        primitive_types: building.primitive_types,
        contexts: building.contexts,
        function_definitions,
    })
}
