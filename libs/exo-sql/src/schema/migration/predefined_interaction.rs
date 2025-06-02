#[cfg(feature = "interactive-migration")]
use std::path::PathBuf;
use std::sync::Mutex;

use crate::SchemaObjectName;

use super::{
    interaction::{InteractionError, TableAction},
    MigrationInteraction,
};

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

    #[cfg(feature = "interactive-migration")]
    pub fn from_file(file_name: &PathBuf) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct InteractionSer {
            #[serde(rename = "rename-table")]
            rename_tables: Option<Vec<RenameTable>>,
            #[serde(rename = "delete-table")]
            delete_tables: Option<Vec<String>>,
            #[serde(rename = "defer-table")]
            defer_tables: Option<Vec<String>>,
        }

        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RenameTable {
            #[serde(rename = "old-table")]
            old_table: String,
            #[serde(rename = "new-table")]
            new_table: String,
        }

        let interaction = std::fs::read_to_string(file_name)
            .map_err(|e| format!("Failed to read interaction file: {}", e))?;

        let interaction = toml::from_str::<InteractionSer>(&interaction)
            .map_err(|e| format!("Failed to parse interaction file: {}", e))?;

        let mut table_actions = vec![];

        fn string_to_table_name(name: &str) -> SchemaObjectName {
            let parts = name.split('.').collect::<Vec<_>>();

            if parts.len() == 1 {
                SchemaObjectName {
                    schema: None,
                    name: parts[0].to_string(),
                }
            } else if parts.len() == 2 {
                SchemaObjectName {
                    schema: Some(parts[0].to_string()),
                    name: parts[1].to_string(),
                }
            } else {
                panic!("Invalid table name: {}", name)
            }
        }

        if let Some(rename_tables) = interaction.rename_tables {
            for rename_table in rename_tables {
                table_actions.push(TableAction::Rename {
                    old_table: string_to_table_name(&rename_table.old_table),
                    new_table: string_to_table_name(&rename_table.new_table),
                });
            }
        }

        if let Some(delete_tables) = interaction.delete_tables {
            for delete_table in delete_tables {
                table_actions.push(TableAction::Delete(string_to_table_name(&delete_table)));
            }
        }

        if let Some(defer_tables) = interaction.defer_tables {
            for defer_table in defer_tables {
                table_actions.push(TableAction::Defer(string_to_table_name(&defer_table)));
            }
        }

        Ok(PredefinedMigrationInteraction::new(table_actions))
    }
}

impl MigrationInteraction for PredefinedMigrationInteraction {
    fn handle_start(&self) {}

    fn handle_table_delete(
        &self,
        deleted_table: &SchemaObjectName,
        _create_tables: &[&SchemaObjectName],
    ) -> Result<TableAction, InteractionError> {
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
            .ok_or_else(|| InteractionError::TableActionNotFound(deleted_table.clone()))?;
        let table_action = actions.remove(action_index);
        Ok(table_action)
    }
}
