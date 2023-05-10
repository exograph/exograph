// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use self::{
    create::CreateCommandDefinition, import::ImportCommandDefinition,
    migrate::MigrateCommandDefinition, verify::VerifyCommandDefinition,
};

use super::command::SubcommandDefinition;

pub(crate) mod create;
pub(crate) mod import;
pub(crate) mod migrate;
pub(crate) mod migration;
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
