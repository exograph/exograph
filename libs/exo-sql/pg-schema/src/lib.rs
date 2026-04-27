// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod column_spec;
mod database_spec;
mod enum_spec;
mod function_spec;
mod index_spec;
mod issue;
mod migration;
mod op;
mod spec;
mod table_spec;
mod trigger_spec;

mod column_default_schema;
mod column_type_schema;
mod constraint;

pub use column_spec::{ColumnReferenceSpec, ColumnSpec, physical_column_type_from_string};
pub use database_spec::DatabaseSpec;
pub use enum_spec::EnumSpec;
pub use issue::WithIssues;
pub use migration::{
    InteractionError, Migration, MigrationError, MigrationInteraction, MigrationStatement,
    PredefinedMigrationInteraction, TableAction, VerificationErrors, migrate_interactively,
    wipe_database,
};
pub use spec::{MigrationScope, MigrationScopeMatches, NameMatching};
pub use table_spec::TableSpec;

/// Trait for types that can print debug information to a writer
#[allow(dead_code)]
pub(crate) trait DebugPrintTo {
    fn debug_print_to<W: std::io::Write>(
        &self,
        writer: &mut W,
        indent: usize,
    ) -> std::io::Result<()>;

    /// Default implementation that writes to stdout
    fn debug_print(&self, indent: usize) {
        self.debug_print_to(&mut std::io::stdout(), indent)
            .expect("Failed to write debug output to stdout");
    }
}
