use std::sync::Mutex;

use crate::{
    schema::{
        database_spec::DatabaseSpec,
        op::SchemaOp,
        spec::{diff, MigrationScope},
    },
    SchemaObjectName,
};

use super::core::{Migration, MigrationError};

#[derive(Debug)]
pub enum TableAction {
    Defer(SchemaObjectName),
    Rename(SchemaObjectName, SchemaObjectName),
    Delete(SchemaObjectName),
}

impl TableAction {
    pub fn target_table(&self) -> &SchemaObjectName {
        match self {
            TableAction::Defer(table) => table,
            TableAction::Rename(old_table, _) => old_table,
            TableAction::Delete(table) => table,
        }
    }
}
pub trait MigrationInteraction: Send + Sync {
    // Print a message to the user, for example to explain that we are starting the migration.
    fn handle_start(&self);

    fn handle_table_delete(
        &self,
        deleted_table: &SchemaObjectName,
        create_tables: Vec<&SchemaObjectName>,
    ) -> Result<TableAction, MigrationError>;
}

async fn get_table_actions(
    old_db_spec: &DatabaseSpec,
    new_db_spec: &DatabaseSpec,
    scope: &MigrationScope,
    interactions: &dyn MigrationInteraction,
) -> Result<Vec<TableAction>, MigrationError> {
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
                    if let TableAction::Rename(_, new_table) = action {
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
                create_tables.iter().map(|table| &table.name).collect(),
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
) -> Result<Migration, MigrationError> {
    let table_actions = get_table_actions(&old_db_spec, &new_db_spec, scope, interactions).await?;

    apply_table_actions(&mut old_db_spec, new_db_spec, table_actions, scope)
}

fn apply_table_actions(
    old_db_spec: &mut DatabaseSpec,
    new_db_spec: DatabaseSpec,
    table_actions: Vec<TableAction>,
    scope: &MigrationScope,
) -> Result<Migration, MigrationError> {
    let mut all_ops: Vec<SchemaOp> = vec![];

    for table_action in table_actions.iter() {
        if let TableAction::Rename(old_table, new_table) = table_action {
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
pub struct PredefinedMigrationInteraction {
    actions: Mutex<Vec<TableAction>>,
}

impl PredefinedMigrationInteraction {
    pub fn new(actions: Vec<TableAction>) -> Self {
        Self {
            actions: Mutex::new(actions),
        }
    }
}

impl MigrationInteraction for PredefinedMigrationInteraction {
    fn handle_start(&self) {}

    fn handle_table_delete(
        &self,
        deleted_table: &SchemaObjectName,
        _create_tables: Vec<&SchemaObjectName>,
    ) -> Result<TableAction, MigrationError> {
        // Find the table action for the deleted table and remove it from the list. This ensures that we don't handle the same table twice.
        let mut actions = self.actions.lock().unwrap();
        let action_index = actions
            .iter()
            .enumerate()
            .find_map(|(index, action)| {
                if action.target_table() == deleted_table {
                    Some(index)
                } else {
                    None
                }
            })
            .unwrap();
        let table_action = actions.remove(action_index);
        Ok(table_action)
    }
}
