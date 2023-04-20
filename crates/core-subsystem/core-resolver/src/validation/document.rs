// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::OperationType;

use super::operation::ValidatedOperation;

/// The validated query document.
#[derive(Debug)]
pub struct ValidatedDocument {
    pub operations: Vec<ValidatedOperation>,
    pub operation_typ: OperationType,
}
