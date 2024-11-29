// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{ast::ast_types::AstExpr, typechecker::Typed};
use core_plugin_shared::interception::{InterceptorIndex, InterceptorKind};
use core_plugin_shared::serializable_system::{SerializableGraphQLBytes, SerializableRestBytes};

pub struct GraphQLSubsystemBuild {
    pub id: String,
    pub serialized_subsystem: SerializableGraphQLBytes,
    pub query_names: Vec<String>,
    pub mutation_names: Vec<String>,
    pub interceptions: Vec<Interception>,
}

#[derive(Debug)]
pub struct Interception {
    pub expr: AstExpr<Typed>,
    pub kind: InterceptorKind,
    pub index: InterceptorIndex,
}

pub struct RestSubsystemBuild {
    pub serialized_subsystem: SerializableRestBytes,
}
