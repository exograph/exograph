// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub fn collection_query_description(entity_name: &str) -> String {
    format!(
        "Get multiple `{entity_name}`s given the provided `where` filter, order by, limit, and offset"
    )
}

pub fn pk_query_description(entity_name: &str) -> String {
    format!("Get a single `{entity_name}` by primary key")
}

pub fn unique_query_description(entity_name: &str, constraint_name: &str) -> String {
    let readable_name = constraint_name.replace('_', " ");
    format!("Get a single `{entity_name}` by {readable_name}")
}

pub fn pk_delete_description(entity_name: &str) -> String {
    format!("Delete a single `{entity_name}` by primary key")
}

pub fn unique_delete_description(entity_name: &str, constraint_name: &str) -> String {
    let readable_name = constraint_name.replace('_', " ");
    format!("Delete a single `{entity_name}` by {readable_name}")
}

pub fn collection_delete_description(entity_name: &str) -> String {
    format!("Delete multiple `{entity_name}`s given the provided `where` filter")
}

pub fn pk_update_description(entity_name: &str) -> String {
    format!(
        "Update a single `{entity_name}` by primary key with the provided data. Any fields not provided will remain unchanged"
    )
}

pub fn unique_update_description(entity_name: &str, constraint_name: &str) -> String {
    let readable_name = constraint_name.replace('_', " ");
    format!(
        "Update a single `{entity_name}` by {readable_name} with the provided data. Any fields not provided will remain unchanged"
    )
}

pub fn collection_update_description(entity_name: &str) -> String {
    format!(
        "Update multiple `{entity_name}`s matching the provided `where` filter with the provided data. Any fields not provided will remain unchanged"
    )
}

pub fn single_create_description(entity_name: &str) -> String {
    format!("Create a new `{entity_name}` with the provided data")
}

pub fn collection_create_description(entity_name: &str) -> String {
    format!("Create multiple `{entity_name}`s with the provided data")
}
