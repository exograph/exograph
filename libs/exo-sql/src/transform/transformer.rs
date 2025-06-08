// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use tracing::instrument;

use crate::{
    AbstractOrderBy, AbstractPredicate, ColumnId, Database,
    asql::{
        abstract_operation::AbstractOperation, delete::AbstractDelete, insert::AbstractInsert,
        select::AbstractSelect, update::AbstractUpdate,
    },
    sql::{
        order::OrderBy,
        predicate::ConcretePredicate,
        select::Select,
        transaction::{TransactionScript, TransactionStepId},
    },
};

use super::pg::{Postgres, selection_level::SelectionLevel};

/// Transform an abstract operation into a transaction script
pub trait OperationTransformer {
    fn to_transaction_script<'a>(
        &self,
        database: &'a Database,
        abstract_operation: AbstractOperation,
    ) -> TransactionScript<'a>;
}

impl OperationTransformer for Postgres {
    fn to_transaction_script<'a>(
        &self,
        database: &'a Database,
        abstract_operation: AbstractOperation,
    ) -> TransactionScript<'a> {
        match abstract_operation {
            AbstractOperation::Select(select) => {
                SelectTransformer::to_transaction_script(self, select, database)
            }
            AbstractOperation::Delete(delete) => {
                DeleteTransformer::to_transaction_script(self, delete, database)
            }
            AbstractOperation::Insert(insert) => {
                InsertTransformer::to_transaction_script(self, insert, None, database)
            }
            AbstractOperation::Update(update) => {
                UpdateTransformer::to_transaction_script(self, update, database)
            }
        }
    }
}

pub trait SelectTransformer {
    fn to_select(&self, abstract_select: AbstractSelect, database: &Database) -> Select;

    fn to_transaction_script<'a>(
        &self,
        abstract_select: AbstractSelect,
        database: &'a Database,
    ) -> TransactionScript<'a>;
}

pub trait DeleteTransformer {
    #[instrument(
        name = "DeleteTransformer::to_transaction_script for Postgres"
        skip(self, database)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_delete: AbstractDelete,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let mut transaction_script = TransactionScript::default();

        self.update_transaction_script(abstract_delete, database, &mut transaction_script);

        transaction_script
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_delete: AbstractDelete,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    );
}

pub trait InsertTransformer {
    #[instrument(
        name = "InsertTransformer::to_transaction_script for Postgres"
        skip(self, parent_step, database)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_insert: AbstractInsert,
        parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let mut transaction_script = TransactionScript::default();

        self.update_transaction_script(
            abstract_insert,
            parent_step,
            database,
            &mut transaction_script,
        );

        transaction_script
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_insert: AbstractInsert,
        parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    );
}

pub trait UpdateTransformer {
    #[instrument(
        name = "UpdateTransformer::to_transaction_script for Postgres"
        skip(self, database)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_update: AbstractUpdate,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let mut transaction_script = TransactionScript::default();

        self.update_transaction_script(abstract_update, database, &mut transaction_script);

        transaction_script
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_update: AbstractUpdate,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    );
}

pub trait PredicateTransformer {
    /// Transform an abstract predicate into a concrete predicate
    ///
    /// # Arguments
    /// * `predicate` - The predicate to transform
    /// * `selection_level` - The selection level of that led to this predicate (through subselects)
    /// * `tables_supplied` - Whether the tables are already in context. If they are, the predicate can simply use the table.column syntax.
    ///   If they are not, the predicate will need to bring in the tables being referred to.
    /// * `database` - The database
    fn to_predicate(
        &self,
        predicate: &AbstractPredicate,
        selection_level: &SelectionLevel,
        assume_tables_in_context: bool,
        database: &Database,
    ) -> ConcretePredicate;
}

pub trait OrderByTransformer {
    fn to_order_by(
        &self,
        order_by: &AbstractOrderBy,
        selection_level: &SelectionLevel,
        database: &Database,
    ) -> OrderBy;
}
