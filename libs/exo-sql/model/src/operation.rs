// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::operation::DatabaseExtension;

use crate::{
    delete::AbstractDelete, insert::AbstractInsert, select::AbstractSelect, update::AbstractUpdate,
};

/// Top-level abstract operation. A transformed version of this can be submitted to a database.
#[derive(Debug)]
pub enum AbstractOperation<Ext: DatabaseExtension> {
    Select(AbstractSelect<Ext>),
    Delete(AbstractDelete<Ext>),
    Insert(AbstractInsert<Ext>),
    Update(AbstractUpdate<Ext>),
}
