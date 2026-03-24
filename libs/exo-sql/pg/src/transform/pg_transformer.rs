// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Postgres-specific transformer traits for mutation operations.
//! These deal with TransactionScript which is a pg-specific concept.

use crate::{
    PgAbstractDelete, PgAbstractInsert, PgAbstractSelect, PgAbstractUpdate,
    transaction::{TransactionScript, TransactionStepId},
};
use exo_sql_core::{ColumnId, Database};

use tracing::instrument;

pub trait PgSelectTransformer {
    fn to_transaction_script<'a>(
        &self,
        abstract_select: PgAbstractSelect,
        database: &'a Database,
    ) -> TransactionScript<'a>;
}

pub trait PgDeleteTransformer {
    #[instrument(
        name = "PgDeleteTransformer::to_transaction_script"
        skip(self, database)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_delete: PgAbstractDelete,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let mut transaction_script = TransactionScript::default();

        self.update_transaction_script(abstract_delete, database, &mut transaction_script);

        transaction_script
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_delete: PgAbstractDelete,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    );
}

pub trait PgInsertTransformer {
    #[instrument(
        name = "PgInsertTransformer::to_transaction_script"
        skip(self, parent_step, database)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_insert: PgAbstractInsert,
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
        abstract_insert: PgAbstractInsert,
        parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    );
}

pub trait PgUpdateTransformer {
    #[instrument(
        name = "PgUpdateTransformer::to_transaction_script"
        skip(self, database)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_update: PgAbstractUpdate,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let mut transaction_script = TransactionScript::default();

        self.update_transaction_script(abstract_update, database, &mut transaction_script);

        transaction_script
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_update: PgAbstractUpdate,
        database: &'a Database,
        transaction_script: &mut TransactionScript<'a>,
    );
}
