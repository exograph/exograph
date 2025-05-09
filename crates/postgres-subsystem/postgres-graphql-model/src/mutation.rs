// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use postgres_core_model::predicate::PredicateParameter;
use serde::{Deserialize, Serialize};

use crate::types::MutationType;
use core_model::mapped_arena::SerializableSlabIndex;
use core_model::type_normalization::{Parameter, Type};
use core_model::types::{FieldType, Named, TypeValidation};

use super::operation::{OperationParameters, PostgresOperation};

/// A mutation such as `createTodo`, `updateTodo`, or `deleteTodo`
pub type PostgresMutation = PostgresOperation<PostgresMutationParameters>;

/// Mutation parameters
#[derive(Serialize, Deserialize, Debug)]
pub enum PostgresMutationParameters {
    /// Parameters for a create mutation such as `createTodo` or `createTodos`
    /// The only parameter is the data to be created such as `data: { title: "New title" }`
    /// This allows mutations such as `{ createTodo(data: { title: "New title" }) }` and
    /// `{ createTodos(data: [{ title: "New title" }, { title: "Another title" }]) }`
    Create(DataParameter),

    /// Parameters for a delete mutation such as `deleteTodo` or `deleteTodos`
    /// The only parameter is a predicate such as `id: 1` or `where: {complete: {eq: true}}`
    /// `{ deleteTodo(id: 1)` or `{ deleteTodos(where: { complete: {eq: true }}) }`
    Delete(Vec<PredicateParameter>),

    /// Parameters for an update mutation such as `updateTodo` or `updateTodos`
    /// It takes two parameters: a predicate such as `id: 1` or `where: {complete: {eq: true}}`
    /// and the data to be updated such as `data: { title: "New title" }`.
    /// This allows mutations such as `{ updateTodo(id: 1, data: { title: "New title" }) }` and
    /// `{ updateTodos(where: { complete: {eq: true }}, data: { title: "New title" }) }`
    Update {
        data_param: DataParameter,
        predicate_params: Vec<PredicateParameter>,
    },
}

impl OperationParameters for PostgresMutationParameters {
    fn introspect(&self) -> Vec<&dyn Parameter> {
        match &self {
            PostgresMutationParameters::Create(data_param) => vec![data_param],
            PostgresMutationParameters::Delete(predicate_params) => predicate_params
                .iter()
                .map(|p| p as &dyn Parameter)
                .collect(),
            PostgresMutationParameters::Update {
                data_param,
                predicate_params,
            } => {
                let mut params: Vec<&dyn Parameter> = predicate_params
                    .iter()
                    .map(|p| p as &dyn Parameter)
                    .collect();
                params.push(data_param);
                params
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DataParameter {
    /// Name of the parameter (typically `data`).
    pub name: String,
    /// Type of the parameter.
    /// Multiple data input will be represented by the [`FieldType::List`] variant for mutations like `createTodos` (note the plural).
    pub typ: FieldType<DataParameterType>,
    /// Type validation for the parameter.
    pub type_validation: Option<TypeValidation>,
    /// Doc comments for the parameter.
    pub doc_comments: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DataParameterType {
    /// The name of the type such as `TodoCreateInput` and `TodoUpdateInput`.
    /// We only need this field to support the current introspection setup
    /// This must be the same as the name of the `MutationType` referenced by the `type_id` field.
    pub name: String,
    /// The id of the type such as `TodoCreateInput`.
    pub type_id: SerializableSlabIndex<MutationType>,
    /// Type validation for the type.
    pub type_validation: Option<TypeValidation>,
}

impl Named for DataParameterType {
    fn name(&self) -> &str {
        &self.name
    }
}

impl Parameter for DataParameter {
    fn name(&self) -> &str {
        &self.name
    }

    fn typ(&self) -> Type {
        (&self.typ).into()
    }

    fn type_validation(&self) -> Option<TypeValidation> {
        None
    }
}
