use crate::{
    SchemaObjectName,
    schema::{
        database_spec::DatabaseSpec,
        op::SchemaOp,
        spec::{MigrationScope, diff},
    },
};

use super::core::Migration;

#[derive(Debug)]
pub enum TableAction {
    Defer(SchemaObjectName),
    Rename {
        old_table: SchemaObjectName,
        new_table: SchemaObjectName,
    },
    Delete(SchemaObjectName),
}

impl TableAction {
    pub fn target_table(&self) -> &SchemaObjectName {
        match self {
            TableAction::Defer(table) => table,
            TableAction::Rename { old_table, .. } => old_table,
            TableAction::Delete(table) => table,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InteractionError {
    #[error("Table action for table {0:?} not found")]
    TableActionNotFound(SchemaObjectName),

    #[error("Generic error: {0}")]
    Generic(String),
}

pub trait MigrationInteraction: Send + Sync {
    // Print a message to the user, for example to explain that we are starting a migration.
    fn handle_start(&self);

    fn handle_table_delete(
        &self,
        deleted_table: &SchemaObjectName,
        create_tables: &[&SchemaObjectName],
    ) -> Result<TableAction, InteractionError>;
}

async fn get_table_actions(
    old_db_spec: &DatabaseSpec,
    new_db_spec: &DatabaseSpec,
    scope: &MigrationScope,
    interactions: &dyn MigrationInteraction,
) -> Result<Vec<TableAction>, InteractionError> {
    let mut table_actions: Vec<TableAction> = vec![];

    loop {
        let diffs = diff(old_db_spec, new_db_spec, scope);

        let create_tables = diffs
            .iter()
            .filter_map(|diff| match diff {
                SchemaOp::CreateTable { table } => Some(*table),
                _ => None,
            })
            .filter(|table| {
                table_actions.iter().all(|action| {
                    if let TableAction::Rename { new_table, .. } = action {
                        new_table != &table.name
                    } else {
                        true
                    }
                })
            })
            .collect::<Vec<_>>();

        let delete_tables = diffs
            .iter()
            .filter_map(|diff| match diff {
                SchemaOp::DeleteTable { table } => Some(table),
                _ => None,
            })
            .filter(|table| {
                !table_actions
                    .iter()
                    .any(|action| action.target_table() == &table.name)
            })
            .collect::<Vec<_>>();

        if delete_tables.is_empty() {
            return Ok(table_actions);
        } else {
            interactions.handle_start();

            let table_action = interactions.handle_table_delete(
                &delete_tables[0].name,
                &create_tables
                    .iter()
                    .map(|table| &table.name)
                    .collect::<Vec<_>>(),
            )?;

            table_actions.push(table_action);
        }
    }
}

pub async fn migrate_interactively(
    mut old_db_spec: DatabaseSpec,
    new_db_spec: DatabaseSpec,
    scope: &MigrationScope,
    interactions: &dyn MigrationInteraction,
) -> Result<Migration, InteractionError> {
    let table_actions = get_table_actions(&old_db_spec, &new_db_spec, scope, interactions).await?;

    apply_table_actions(&mut old_db_spec, new_db_spec, table_actions, scope)
}

fn apply_table_actions(
    old_db_spec: &mut DatabaseSpec,
    new_db_spec: DatabaseSpec,
    table_actions: Vec<TableAction>,
    scope: &MigrationScope,
) -> Result<Migration, InteractionError> {
    let mut all_ops: Vec<SchemaOp> = vec![];

    for table_action in table_actions.iter() {
        if let TableAction::Rename {
            old_table,
            new_table,
        } = table_action
        {
            let rename_ops = old_db_spec.with_table_renamed(old_table, new_table);
            all_ops.extend(rename_ops);
        }
    }

    let diffs = diff(old_db_spec, &new_db_spec, scope);

    let diffs = diffs
        .into_iter()
        .map(|diff| {
            let allow_destructive = table_actions.iter().any(|action|
                matches!((&diff, action), (SchemaOp::DeleteTable { table }, TableAction::Delete(table_name)) if table_name == &table.name)
            );

            (diff, if allow_destructive { Some(false) } else { None })
        })
        .collect::<Vec<_>>();

    let all_ops = all_ops
        .into_iter()
        .map(|op| (op, None))
        .chain(diffs)
        .collect::<Vec<_>>();

    Ok(Migration::from_diffs(&all_ops))
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct AlwaysDeferMigrationInteraction;

impl MigrationInteraction for AlwaysDeferMigrationInteraction {
    fn handle_start(&self) {}

    fn handle_table_delete(
        &self,
        deleted_table: &SchemaObjectName,
        _create_tables: &[&SchemaObjectName],
    ) -> Result<TableAction, InteractionError> {
        Ok(TableAction::Defer(deleted_table.clone()))
    }
}
