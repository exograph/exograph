// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::{hash_map::RandomState, hash_set::Difference};

use super::{database_spec::DatabaseSpec, op::SchemaOp};

pub fn diff<'a>(old: &'a DatabaseSpec, new: &'a DatabaseSpec) -> Vec<SchemaOp<'a>> {
    let mut changes = vec![];

    let old_required_extensions = old.required_extensions();
    let new_required_extensions = new.required_extensions();

    // extension creation
    let extensions_to_create =
        sorted_strings(new_required_extensions.difference(&old_required_extensions));
    for extension in extensions_to_create {
        changes.push(SchemaOp::CreateExtension {
            extension: extension.clone(),
        })
    }

    let old_required_schemas = old.required_schemas();
    let new_required_schemas = new.required_schemas();

    // schema creation
    let schemas_to_create = sorted_strings(new_required_schemas.difference(&old_required_schemas));
    for schema in schemas_to_create {
        changes.push(SchemaOp::CreateSchema {
            schema: schema.clone(),
        })
    }

    for old_table in old.tables.iter() {
        // try to find a table with the same name in the new spec
        match new
            .tables
            .iter()
            .find(|new_table| old_table.sql_name() == new_table.sql_name())
        {
            // table exists, compare columns
            Some(new_table) => changes.extend(old_table.diff(new_table)),

            // table does not exist, deletion
            None => changes.push(SchemaOp::DeleteTable { table: old_table }),
        }
    }

    // try to find a table that needs to be created
    for new_table in new.tables.iter() {
        if !old
            .tables
            .iter()
            .any(|old_table| new_table.sql_name() == old_table.sql_name())
        {
            // new table
            changes.push(SchemaOp::CreateTable { table: new_table })
        }
    }

    for old_function in old.functions.iter() {
        // try to find a function with the same name in the new spec
        match new
            .functions
            .iter()
            .find(|new_function| old_function.name == new_function.name)
        {
            // function exists, compare bodies
            Some(new_function) => changes.extend(old_function.diff(new_function)),

            // function does not exist, deletion
            None => changes.push(SchemaOp::DeleteFunction {
                name: &old_function.name,
            }),
        }
    }

    // try to find a function that needs to be created
    for new_function in new.functions.iter() {
        if !old
            .functions
            .iter()
            .any(|old_function| new_function.name == old_function.name)
        {
            // new function
            changes.push(SchemaOp::CreateFunction {
                function: new_function,
            })
        }
    }

    // extension removal
    let extensions_to_drop =
        sorted_strings(old_required_extensions.difference(&new_required_extensions));
    for extension in extensions_to_drop {
        changes.push(SchemaOp::RemoveExtension {
            extension: extension.clone(),
        })
    }

    // schema removal
    let schemas_to_drop = sorted_strings(old_required_schemas.difference(&new_required_schemas));
    for schema in schemas_to_drop {
        changes.push(SchemaOp::DeleteSchema {
            schema: schema.clone(),
        })
    }

    // sort changes so that triggers are created after its functions
    changes.sort_by(|a, b| match (a, b) {
        (SchemaOp::CreateTrigger { .. }, SchemaOp::CreateFunction { .. }) => {
            std::cmp::Ordering::Greater
        }
        (SchemaOp::CreateFunction { .. }, SchemaOp::CreateTrigger { .. }) => {
            std::cmp::Ordering::Less
        }
        _ => std::cmp::Ordering::Equal,
    });

    changes
}

fn sorted_strings(strings: Difference<String, RandomState>) -> Vec<&String> {
    let mut strings: Vec<_> = strings.into_iter().collect();
    strings.sort();
    strings
}
