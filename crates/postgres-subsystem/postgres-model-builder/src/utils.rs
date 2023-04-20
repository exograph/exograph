// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::types::FieldType;
use postgres_model::types::{EntityType, MutationType, PostgresFieldType, TypeIndex};

pub(super) fn to_mutation_type(
    field_type: &FieldType<PostgresFieldType<EntityType>>,
) -> FieldType<PostgresFieldType<MutationType>> {
    match field_type {
        FieldType::Optional(ft) => FieldType::Optional(Box::new(to_mutation_type(ft))),
        FieldType::Plain(PostgresFieldType { type_id, type_name }) => match type_id {
            TypeIndex::Primitive(index) => FieldType::Plain(PostgresFieldType {
                type_id: TypeIndex::Primitive(*index),
                type_name: type_name.clone(),
            }),
            TypeIndex::Composite(_) => panic!(),
        },
        FieldType::List(ft) => FieldType::List(Box::new(to_mutation_type(ft))),
    }
}
