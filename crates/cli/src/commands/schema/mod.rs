use self::{
    create::CreateCommandDefinition, import::ImportCommandDefinition,
    migrate::MigrateCommandDefinition, verify::VerifyCommandDefinition,
};

use super::command::SubcommandDefinition;

pub(crate) mod create;
pub(crate) mod import;
pub(crate) mod migrate;
pub(crate) mod migration_helper;
pub(crate) mod util;
pub(crate) mod verify;

pub fn command_definition() -> SubcommandDefinition {
    SubcommandDefinition::new(
        "schema",
        "Create, migrate, verify, and import  database schema",
        vec![
            Box::new(CreateCommandDefinition {}),
            Box::new(MigrateCommandDefinition {}),
            Box::new(VerifyCommandDefinition {}),
            Box::new(ImportCommandDefinition {}),
        ],
    )
}
