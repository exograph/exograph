// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use heck::ToSnakeCase;

pub fn get_single(entity_name: &str) -> String {
    format!("get_{}", entity_name.to_snake_case())
}

pub fn get_collection(plural_name: &str) -> String {
    format!("get_{}", plural_name.to_snake_case())
}

pub fn delete_single(entity_name: &str) -> String {
    format!("delete_{}", entity_name.to_snake_case())
}

pub fn delete_collection(plural_name: &str) -> String {
    format!("delete_{}", plural_name.to_snake_case())
}

pub fn get_single_by_unique(entity_name: &str, constraint_name: &str) -> String {
    format!(
        "get_{}_by_{}",
        entity_name.to_snake_case(),
        constraint_name.to_snake_case()
    )
}

pub fn delete_single_by_unique(entity_name: &str, constraint_name: &str) -> String {
    format!(
        "delete_{}_by_{}",
        entity_name.to_snake_case(),
        constraint_name.to_snake_case()
    )
}

pub fn update_single(entity_name: &str) -> String {
    format!("update_{}", entity_name.to_snake_case())
}

pub fn update_collection(plural_name: &str) -> String {
    format!("update_{}", plural_name.to_snake_case())
}

pub fn update_single_by_unique(entity_name: &str, constraint_name: &str) -> String {
    format!(
        "update_{}_by_{}",
        entity_name.to_snake_case(),
        constraint_name.to_snake_case()
    )
}

pub fn create_single(entity_name: &str) -> String {
    format!("create_{}", entity_name.to_snake_case())
}

pub fn create_collection(plural_name: &str) -> String {
    format!("create_{}", plural_name.to_snake_case())
}
