// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::OperationType;
use core_resolver::introspection::definition::schema::Schema;

#[derive(Debug)]
pub struct IntrospectionRootElement<'a> {
    pub schema: &'a Schema,
    pub operation_type: &'a OperationType,
    pub name: &'a str,
}
