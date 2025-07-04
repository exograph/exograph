// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub mod column_spec;
pub mod database_spec;
pub mod enum_spec;
pub mod function_spec;
pub mod index_spec;
pub mod issue;
pub mod migration;
pub mod op;
pub mod spec;
pub mod table_spec;
pub mod test_helper;
pub mod trigger_spec;

mod constraint;
pub mod statement;

/// Trait for types that can print debug information to a writer
pub trait DebugPrintTo {
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
