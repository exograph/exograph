// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub fn get_single(entity_name: &str) -> String {
    format!("get_{}", entity_name.to_lowercase())
}

pub fn get_collection(plural_name: &str) -> String {
    format!("get_{}", plural_name.to_lowercase())
}

pub fn delete_single(entity_name: &str) -> String {
    format!("delete_{}", entity_name.to_lowercase())
}

pub fn delete_collection(plural_name: &str) -> String {
    format!("delete_{}", plural_name.to_lowercase())
}
