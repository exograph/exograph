// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::physical_column::ColumnId;

/// A function applied to a column. For example, `count(id)` or `lower(first_name)`.
#[derive(Debug, PartialEq, Clone)]
pub enum Function<Ext> {
    Named {
        function_name: String,
        column_id: ColumnId,
    },
    Extension(Ext),
}
