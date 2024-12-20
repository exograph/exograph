// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::Pos;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("{0}")]
    QueryParsingFailed(String, Pos, Option<Pos>),

    #[error("Variable '{0}' not found")]
    VariableNotFound(String, Pos),

    #[error("Variable '{0}' could not be deserialized: {2}")]
    MalformedVariable(String, Pos, serde_json::Error),

    #[error("Fragment definition '{0}' not found")]
    FragmentDefinitionNotFound(String, Pos),

    #[error("Inline fragments are not supported")]
    InlineFragmentNotSupported(Pos),

    #[error("No such operation '{0}'")]
    OperationNotFound(String, Pos),

    #[error("Field '{0}' is not valid for type '{1}'")]
    InvalidField(String, String, Pos),

    #[error("Field '{0}' is of a scalar type, which should not specify fields")]
    ScalarWithField(String, Pos),

    #[error("Field type '{0}' is not valid")]
    InvalidFieldType(String, Pos),

    #[error("Required argument '{0}' not found")]
    RequiredArgumentNotFound(String, Pos),

    #[error("Argument(s) '{0:?}' invalid for '{1}'")]
    StrayArguments(Vec<String>, String, Pos),

    #[error(
        "Argument '{argument_name}' is not of a valid type. Expected '{expected_type}', got '{actual_type}'"
    )]
    InvalidArgumentType {
        argument_name: String,
        expected_type: String,
        actual_type: String,
        pos: Pos,
    },

    #[error(
        "Failed to merge field defined multiple times with different selection or arguments: '{0}'"
    )]
    MergeFields(String, Vec<Pos>),

    #[error("No operation found")]
    NoOperationFound,

    #[error("Must provide operation name if query contains multiple operations")]
    MultipleOperationsNoOperationName,

    #[error("operationName '{0}' doesn't match any operation")]
    MultipleOperationsUnmatchedOperationName(String),

    #[error("Fragment cycle detected: {0}")]
    FragmentCycle(String, Pos),

    #[error("Selection set too deep")]
    SelectionSetTooDeep(Pos),

    #[error("Invalid value for '{value_name}': {range_detail}, {value_detail}")]
    ValueOutOfRange {
        value_name: String,
        range_detail: String,
        value_detail: String,
        pos: Pos,
    },
}

impl ValidationError {
    pub fn positions(&self) -> Vec<Pos> {
        match self {
            ValidationError::QueryParsingFailed(_, pos1, pos2) => {
                vec![Some(*pos1), *pos2].into_iter().flatten().collect()
            }
            ValidationError::VariableNotFound(_, pos) => vec![*pos],
            ValidationError::MalformedVariable(_, pos, _) => vec![*pos],
            ValidationError::FragmentDefinitionNotFound(_, pos) => vec![*pos],
            ValidationError::InlineFragmentNotSupported(pos) => vec![*pos],
            ValidationError::OperationNotFound(_, pos) => vec![*pos],
            ValidationError::InvalidField(_, _, pos) => vec![*pos],
            ValidationError::InvalidFieldType(_, pos) => vec![*pos],
            ValidationError::ScalarWithField(_, pos) => vec![*pos],
            ValidationError::RequiredArgumentNotFound(_, pos) => vec![*pos],
            ValidationError::StrayArguments(_, _, pos) => vec![*pos],
            ValidationError::MergeFields(_, pos) => pos.clone(),
            ValidationError::NoOperationFound => vec![],
            ValidationError::MultipleOperationsNoOperationName => vec![],
            ValidationError::MultipleOperationsUnmatchedOperationName(_) => vec![],
            ValidationError::InvalidArgumentType { pos, .. } => vec![*pos],
            ValidationError::FragmentCycle(_, pos) => vec![*pos],
            ValidationError::SelectionSetTooDeep(pos) => vec![*pos],
            ValidationError::ValueOutOfRange { pos, .. } => vec![*pos],
        }
    }
}
