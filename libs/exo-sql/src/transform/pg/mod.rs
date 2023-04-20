// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod delete_transformer;
mod insert_transformer;
mod order_by_transformer;
mod predicate_transformer;
mod select;
mod update_transformer;

pub struct Postgres {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionLevel {
    TopLevel,
    Nested,
}
