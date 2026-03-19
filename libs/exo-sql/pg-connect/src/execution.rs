// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use tokio_postgres::GenericClient;
use tracing::{error, info, instrument};

use exo_sql_core::{Database, database_error::DatabaseError};
use exo_sql_pg_core::{
    ExpressionBuilder, SQLBuilder, SQLOperation,
    transaction::{
        ConcreteTransactionStep, TransactionContext, TransactionScript, TransactionStep,
        TransactionStepResult,
    },
};

/// Execute all steps of a transaction script and return the result of the last step.
#[instrument(
    name = "execute_transaction_script"
    skip_all
)]
pub async fn execute_transaction_script(
    script: TransactionScript<'_>,
    database: &Database,
    tx: &mut impl GenericClient,
) -> Result<TransactionStepResult, DatabaseError> {
    let mut transaction_context = TransactionContext::new();

    // Execute each step in the transaction and store the result in the transaction_context
    for step in script.into_steps().into_iter() {
        let result = execute_transaction_step(step, database, tx, &transaction_context).await?;
        transaction_context.push(result);
    }

    // Return the result of the last step (usually the "select")
    transaction_context
        .into_last_result()
        .ok_or_else(|| DatabaseError::Transaction("".into()))
}

/// Execute a single transaction step.
#[instrument(
    name = "execute_transaction_step"
    level = "trace"
    skip_all
)]
pub async fn execute_transaction_step(
    step: TransactionStep<'_>,
    database: &Database,
    client: &mut impl GenericClient,
    transaction_context: &TransactionContext,
) -> Result<TransactionStepResult, DatabaseError> {
    match step {
        TransactionStep::Concrete(step) => execute_concrete_step(*step, database, client).await,
        TransactionStep::Template(step) => {
            let concrete = step.resolve(transaction_context);

            let mut res: Result<TransactionStepResult, DatabaseError> = Ok(vec![]);

            let substep_count = concrete.len();

            for (index, substep) in concrete.into_iter().enumerate() {
                if index == substep_count - 1 {
                    // Execute the last step and return the result
                    res = execute_concrete_step(substep, database, client).await;
                } else {
                    // Execute all but the last step
                    execute_concrete_step(substep, database, client).await?;
                }
            }

            res
        }
        TransactionStep::Filter(step) => {
            let concrete = step.resolve(transaction_context, database);
            execute_concrete_step(concrete, database, client).await
        }
        TransactionStep::Dynamic(step) => {
            execute_concrete_step(step.resolve(transaction_context), database, client).await
        }
        TransactionStep::Precheck(select) => {
            let precheck_result = run_query(SQLOperation::Select(select), database, client).await?;
            if precheck_result.len() != 1 {
                return Err(DatabaseError::Precheck(format!(
                    "Expected 1 row, got {}",
                    precheck_result.len()
                )));
            }

            Ok(precheck_result)
        }
    }
}

/// Execute a concrete transaction step (a single SQL operation).
#[instrument(
    name = "execute_concrete_step"
    level = "trace"
    skip_all
    fields(
        operation = ?step.operation
    )
)]
pub async fn execute_concrete_step(
    step: ConcreteTransactionStep<'_>,
    database: &Database,
    client: &mut impl GenericClient,
) -> Result<TransactionStepResult, DatabaseError> {
    run_query(step.operation, database, client).await
}

async fn run_query(
    operation: SQLOperation<'_>,
    database: &Database,
    client: &mut impl GenericClient,
) -> Result<TransactionStepResult, DatabaseError> {
    let mut sql_builder = SQLBuilder::new();
    operation.build(database, &mut sql_builder);
    let (stmt, params) = sql_builder.into_sql();

    let params: Vec<_> = params
        .iter()
        .map(|p| (p.param.as_pg(), p.param_type.clone()))
        .collect();

    info!("Executing SQL operation: {}", stmt);

    client.query_typed(&stmt, &params[..]).await.map_err(|e| {
        error!("Failed to execute query: {e:?}");
        DatabaseError::Delegate(e).with_context("Database operation failed".into())
    })
}
