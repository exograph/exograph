// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{database_spec::DatabaseSpec, op::SchemaOp};

pub fn diff<'a>(old: &'a DatabaseSpec, new: &'a DatabaseSpec) -> Vec<SchemaOp<'a>> {
    let mut changes = vec![];

    let old_required_extensions = old.required_extensions();
    let new_required_extensions = new.required_extensions();

    // extension removal
    let extensions_to_drop = old_required_extensions.difference(&new_required_extensions);
    for extension in extensions_to_drop {
        changes.push(SchemaOp::RemoveExtension {
            extension: extension.clone(),
        })
    }

    // extension creation
    let extensions_to_create = new_required_extensions.difference(&old_required_extensions);
    for extension in extensions_to_create {
        changes.push(SchemaOp::CreateExtension {
            extension: extension.clone(),
        })
    }

    for old_table in old.tables.iter() {
        // try to find a table with the same name in the new spec
        match new
            .tables
            .iter()
            .find(|new_table| old_table.name == new_table.name)
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
            .any(|old_table| new_table.name == old_table.name)
        {
            // new table
            changes.push(SchemaOp::CreateTable { table: new_table })
        }
    }

    changes
}
