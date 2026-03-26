// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::Ordering;
use exo_sql_core::operation::{AbstractOrderByExtensionPaths, DatabaseExtension};

use crate::column_path::PhysicalColumnPath;

/// Represents an abstract order-by clause
#[derive(Debug)]
pub struct AbstractOrderBy<Ext: DatabaseExtension>(pub Vec<(AbstractOrderByExpr<Ext>, Ordering)>);

#[derive(Debug)]
pub enum AbstractOrderByExpr<Ext: DatabaseExtension> {
    Column(PhysicalColumnPath),
    Extension(Ext::AbstractOrderByExtension),
}

impl<Ext: DatabaseExtension> AbstractOrderBy<Ext> {
    pub fn column_paths(&self) -> Vec<&PhysicalColumnPath> {
        self.0
            .iter()
            .flat_map(|(expr, _)| match expr {
                AbstractOrderByExpr::Column(path) => vec![path],
                AbstractOrderByExpr::Extension(ext) => ext.physical_column_paths(),
            })
            .collect()
    }
}
