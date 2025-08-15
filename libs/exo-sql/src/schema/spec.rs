// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    collections::{hash_map::RandomState, hash_set::Difference},
    hash::Hash,
};

use wildmatch::WildMatch;

use crate::SchemaObjectName;

use super::{database_spec::DatabaseSpec, op::SchemaOp};

#[derive(Debug, PartialEq)]
pub enum MigrationScope {
    Specified(MigrationScopeMatches),
    FromNewSpec,
}

impl MigrationScope {
    pub fn all_schemas() -> Self {
        Self::Specified(MigrationScopeMatches::all_schemas())
    }
}

#[derive(Debug, PartialEq)]
pub struct MigrationScopeMatches(pub Vec<(NameMatching, NameMatching)>);

impl MigrationScopeMatches {
    pub fn all_schemas() -> Self {
        Self(vec![(NameMatching::new("*"), NameMatching::new("*"))])
    }

    pub fn matches(&self, table_name: &SchemaObjectName) -> bool {
        self.0.iter().any(|(schema_pattern, table_pattern)| {
            schema_pattern.matches(&table_name.schema_name())
                && table_pattern.matches(&table_name.name)
        })
    }

    pub fn matches_schema(&self, schema_name: &str) -> bool {
        self.0
            .iter()
            .any(|(schema_pattern, _)| schema_pattern.matches(schema_name))
    }

    pub fn from_specs_schemas(specs: &[&DatabaseSpec]) -> Self {
        let mut schemas = specs
            .iter()
            .flat_map(|spec| spec.required_schemas(&MigrationScopeMatches::all_schemas()))
            .collect::<Vec<_>>();
        if specs.iter().any(|spec| spec.needs_public_schema()) {
            schemas.push("public".to_string());
        }

        MigrationScopeMatches(
            schemas
                .into_iter()
                .map(|schema| (NameMatching::new(&schema), NameMatching::new("*")))
                .collect(),
        )
    }
}

#[derive(Debug, PartialEq)]
pub struct NameMatching(WildMatch); // matches if the name contains the pattern

impl NameMatching {
    pub fn new(pattern: &str) -> Self {
        Self(WildMatch::new(pattern))
    }

    pub fn matches(&self, name: &str) -> bool {
        self.0.matches(name)
    }
}

pub fn diff<'a>(
    old: &'a DatabaseSpec,
    new: &'a DatabaseSpec,
    scope: &MigrationScope,
) -> Vec<SchemaOp<'a>> {
    let mut changes = vec![];

    let scope_matches = match scope {
        MigrationScope::Specified(spec) => spec,
        MigrationScope::FromNewSpec => &MigrationScopeMatches::from_specs_schemas(&[new]),
    };

    let old_required_extensions = old.required_extensions(scope_matches);
    let new_required_extensions = new.required_extensions(scope_matches);

    // extension creation
    let extensions_to_create =
        sorted_values(new_required_extensions.difference(&old_required_extensions));
    for extension in extensions_to_create {
        changes.push(SchemaOp::CreateExtension {
            extension: extension.clone(),
        })
    }

    let old_required_schemas = old.required_schemas(scope_matches);
    let new_required_schemas = new.required_schemas(scope_matches);

    // schema creation
    let schemas_to_create = sorted_values(new_required_schemas.difference(&old_required_schemas));
    for schema in schemas_to_create {
        changes.push(SchemaOp::CreateSchema {
            schema: schema.clone(),
        })
    }

    let old_required_sequences = old.required_sequences(scope_matches);
    let new_required_sequences = new.required_sequences(scope_matches);

    let sequences_to_create =
        sorted_values(new_required_sequences.difference(&old_required_sequences));
    for sequence in sequences_to_create {
        changes.push(SchemaOp::CreateSequence {
            sequence: sequence.clone(),
        })
    }

    let sequences_to_drop =
        sorted_values(old_required_sequences.difference(&new_required_sequences));
    for sequence in sequences_to_drop {
        changes.push(SchemaOp::DeleteSequence {
            sequence: sequence.clone(),
        })
    }

    let old_enums = old
        .enums
        .iter()
        .filter(|enum_| scope_matches.matches(&enum_.name))
        .collect::<Vec<_>>();
    let new_enums = new
        .enums
        .iter()
        .filter(|enum_| scope_matches.matches(&enum_.name))
        .collect::<Vec<_>>();

    for old_enum in &old_enums {
        // try to find a enum with the same name in the new spec
        match new_enums
            .iter()
            .find(|new_enum| old_enum.sql_name() == new_enum.sql_name())
        {
            Some(new_enum) => changes.extend(old_enum.diff(new_enum)),

            // enum does not exist, deletion
            None => changes.push(SchemaOp::DeleteEnum { enum_: old_enum }),
        }
    }

    for new_enum in new_enums {
        if !old_enums
            .iter()
            .any(|old_enum| new_enum.sql_name() == old_enum.sql_name())
        {
            changes.push(SchemaOp::CreateEnum { enum_: new_enum })
        }
    }

    let old_tables = old
        .tables
        .iter()
        .filter(|table| scope_matches.matches(&table.name))
        .collect::<Vec<_>>();
    let new_tables = new
        .tables
        .iter()
        .filter(|table| scope_matches.matches(&table.name))
        .collect::<Vec<_>>();

    for old_table in &old_tables {
        // try to find a table with the same name in the new spec
        match new_tables
            .iter()
            .find(|new_table| old_table.sql_name() == new_table.sql_name())
        {
            // table exists, compare columns
            Some(new_table) => changes.extend(old_table.diff(new_table)),

            // table does not exist, deletion
            None => changes.push(SchemaOp::DeleteTable { table: old_table }),
        }
    }

    // try to find a table that needs to be created
    for new_table in new_tables.iter().filter(|table| table.managed) {
        if !old_tables
            .iter()
            .any(|old_table| new_table.sql_name() == old_table.sql_name())
        {
            // new table
            changes.push(SchemaOp::CreateTable { table: new_table })
        }
    }

    for old_function in old.functions.iter() {
        // try to find a function with the same name in the new spec
        match new
            .functions
            .iter()
            .find(|new_function| old_function.name == new_function.name)
        {
            // function exists, compare bodies
            Some(new_function) => changes.extend(old_function.diff(new_function)),

            // function does not exist, deletion
            None => changes.push(SchemaOp::DeleteFunction {
                name: &old_function.name,
            }),
        }
    }

    // try to find a function that needs to be created
    for new_function in new.functions.iter() {
        if !old
            .functions
            .iter()
            .any(|old_function| new_function.name == old_function.name)
        {
            // new function
            changes.push(SchemaOp::CreateFunction {
                function: new_function,
            })
        }
    }

    // extension removal
    let extensions_to_drop =
        sorted_values(old_required_extensions.difference(&new_required_extensions));
    for extension in extensions_to_drop {
        changes.push(SchemaOp::RemoveExtension {
            extension: extension.clone(),
        })
    }

    // schema removal
    let schemas_to_drop = sorted_values(old_required_schemas.difference(&new_required_schemas));
    for schema in schemas_to_drop {
        changes.push(SchemaOp::DeleteSchema {
            schema: schema.clone(),
        })
    }

    // sort changes so that triggers are created after its functions
    changes.sort_by(|a, b| match (a, b) {
        (SchemaOp::CreateTrigger { .. }, SchemaOp::CreateFunction { .. }) => {
            std::cmp::Ordering::Greater
        }
        (SchemaOp::CreateFunction { .. }, SchemaOp::CreateTrigger { .. }) => {
            std::cmp::Ordering::Less
        }
        _ => std::cmp::Ordering::Equal,
    });

    changes
}

fn sorted_values<T: Ord + Eq + Hash>(values: Difference<'_, T, RandomState>) -> Vec<&T> {
    let mut strings: Vec<_> = values.into_iter().collect();
    strings.sort();
    strings
}
