use std::path::{Path, PathBuf};

use crate::subsystem::PostgresCoreSubsystem;
use exo_sql::{
    schema::{
        database_spec::DatabaseSpec,
        migration::{
            migrate_interactively, Migration, MigrationStatement, PredefinedMigrationInteraction,
            TableAction,
        },
        spec::MigrationScope,
    },
    SchemaObjectName,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use colored::Colorize;
use wildmatch::WildMatch;

use core_model_builder::plugin::BuildMode;
use core_plugin_shared::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn all_tests() {
    let filter = std::env::var("_EXO_DEV_MIGRATION_TEST_FILTER").unwrap_or("*".to_string());
    let wildcard = WildMatch::new(&filter);

    let test_configs_dir = base_path();
    let test_configs = std::fs::read_dir(test_configs_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .filter(|entry| wildcard.matches(entry.file_name().to_str().unwrap()));

    let mut failed = false;

    for test_config in test_configs {
        let test_config_name = test_config.file_name();
        let test_config_name = test_config_name.to_str().unwrap();
        if let Err(e) = single_test(test_config_name).await {
            println!("{}: {}", test_config_name, "failed".red());
            println!("{}", e);
            failed = true;
        }
    }

    if failed {
        panic!("{}", "Some tests failed".red());
    }
}

async fn single_test<P: AsRef<Path>>(folder: P) -> Result<(), String> {
    let folder = folder.as_ref();
    println!("Testing {}", folder.display());
    let old_exo = read_relative_file(folder, PathBuf::from("old/src/index.exo"))
        .map_err(|e| format!("Failed to read old exo: {}", e))?;
    let new_exo = read_relative_file(folder, PathBuf::from("new/src/index.exo"))
        .map_err(|e| format!("Failed to read new exo: {}", e))?;

    let scope_dirs = std::fs::read_dir(relative_path(folder, PathBuf::from("")))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .filter(|entry| entry.file_name().to_str().unwrap().starts_with("scope-"));

    let mut failed = false;

    for scope_dir in scope_dirs {
        let scope_dir_name = scope_dir.file_name().to_str().unwrap().to_owned();
        let scope_spec_name = scope_dir_name.replace("scope-", "");
        let scope = if scope_spec_name == "all-schemas" {
            Ok(MigrationScope::all_schemas())
        } else if scope_spec_name == "new-spec" {
            Ok(MigrationScope::FromNewSpec)
        } else {
            Err(format!("Unknown scope: {}", scope_dir_name))
        }?;

        let scope_folder = format!("{}/{}", folder.display(), scope_dir_name);

        println!("\tscope {}:", scope_spec_name);

        if let Err(e) = assert_for_scope(&old_exo, &new_exo, &scope_folder, &scope).await {
            println!("{}: {}", scope_folder, e);
            failed = true;
        }
    }

    if failed {
        Err(format!("{}: Some tests failed", folder.display()))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestKind {
    OldCreation,
    NewCreation,
    IdempotentSelfMigration,
    Up,
    Down,
}

impl TestKind {
    fn kind_str(&self) -> &str {
        match self {
            TestKind::OldCreation => "old",
            TestKind::NewCreation => "new",
            TestKind::IdempotentSelfMigration => "idempotent",
            TestKind::Up => "up",
            TestKind::Down => "down",
        }
    }
}

async fn assert_for_scope<P: AsRef<Path>>(
    old_exo: &str,
    new_exo: &str,
    folder: P,
    scope: &MigrationScope,
) -> Result<(), String> {
    let folder = folder.as_ref();

    let old_expected_sql = read_relative_file(folder, PathBuf::from("old.sql")).unwrap_or_default();
    let new_expected_sql = read_relative_file(folder, PathBuf::from("new.sql")).unwrap_or_default();
    let up_expected_sql = read_relative_file(folder, PathBuf::from("up.sql")).unwrap_or_default();
    let down_expected_sql =
        read_relative_file(folder, PathBuf::from("down.sql")).unwrap_or_default();

    let old_system = compute_spec(old_exo).await;
    let new_system = compute_spec(new_exo).await;

    let mut failed = false;

    if let Err(e) = assert_creation_and_self_migration(
        &old_system,
        &old_expected_sql,
        scope,
        folder,
        TestKind::OldCreation,
    ) {
        println!("Old creation failed: {}", e);
        failed = true;
    } else {
        println!("\t\told-creation: {}", "pass".green());
    }

    if let Err(e) = assert_creation_and_self_migration(
        &new_system,
        &new_expected_sql,
        scope,
        folder,
        TestKind::NewCreation,
    ) {
        println!("New creation failed: {}", e);
        failed = true;
    } else {
        println!("\t\tnew-creation: {}", "pass".green());
    }

    if let Err(e) = assert_migration(
        &old_system,
        &new_system,
        &up_expected_sql,
        scope,
        folder,
        TestKind::Up,
    ) {
        println!("Up failed: {}", e);
        failed = true;
    } else {
        println!("\t\tup: {}", "pass".green());
    }

    if let Err(e) = assert_migration(
        &new_system,
        &old_system,
        &down_expected_sql,
        scope,
        folder,
        TestKind::Down,
    ) {
        println!("Down failed: {}", e);
        failed = true;
    } else {
        println!("\t\tdown: {}", "pass".green());
    }

    if let Err(e) = assert_interactive_migrations(old_exo, new_exo, scope, folder).await {
        println!("Interactive migration failed: {}", e);
        failed = true;
    }

    if failed {
        Err(format!(
            "{}: Tests for scope {:?} failed",
            folder.display(),
            scope
        ))
    } else {
        Ok(())
    }
}

fn assert_creation_and_self_migration<P: AsRef<Path>>(
    system: &DatabaseSpec,
    expected: &str,
    migration_scope: &MigrationScope,
    folder: P,
    kind: TestKind,
) -> Result<(), String> {
    let creation = Migration::from_schemas(
        &DatabaseSpec::new(vec![], vec![], vec![]),
        system,
        migration_scope,
    );
    assert_sql_eq(creation, expected, folder.as_ref(), kind.kind_str())?;

    let self_migration = Migration::from_schemas(system, system, migration_scope);
    assert_sql_eq(
        self_migration,
        "",
        folder.as_ref(),
        TestKind::IdempotentSelfMigration.kind_str(),
    )?;

    Ok(())
}

fn assert_migration<P: AsRef<Path>>(
    old_system: &DatabaseSpec,
    new_system: &DatabaseSpec,
    expected: &str,
    migration_scope: &MigrationScope,
    folder: P,
    kind: TestKind,
) -> Result<(), String> {
    let migration = Migration::from_schemas(old_system, new_system, migration_scope);
    assert_sql_eq(migration, expected, folder.as_ref(), kind.kind_str())
}

async fn assert_interactive_migrations<P: AsRef<Path>>(
    old_exo: &str,
    new_exo: &str,
    migration_scope: &MigrationScope,
    folder: P,
) -> Result<(), String> {
    let interactive_dir = relative_path(folder, PathBuf::from("interactive"));

    if !std::path::Path::new(&interactive_dir).exists() {
        return Ok(());
    }

    println!("\t\tinteractive:");

    for kind in [TestKind::Up, TestKind::Down] {
        assert_interactive_migration(old_exo, new_exo, kind, migration_scope, &interactive_dir)
            .await?
    }

    Ok(())
}

async fn assert_interactive_migration(
    old_exo: &str,
    new_exo: &str,
    kind: TestKind,
    scope: &MigrationScope,
    folder: &Path,
) -> Result<(), String> {
    let subfolder = folder.join(kind.kind_str());

    if !std::path::Path::new(&subfolder).exists() {
        return Ok(());
    }

    println!("\t\t\t{}:", kind.kind_str());

    let interaction_file_names = std::fs::read_dir(&subfolder)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().unwrap().is_file())
        .filter(|entry| Path::new(&entry.file_name()).extension().unwrap() == "toml")
        .map(|entry| entry.file_name().to_str().unwrap().to_owned());

    for interaction_file_name in interaction_file_names {
        let interaction_name = interaction_file_name.replace(".toml", "");

        let interaction_file_name = subfolder.clone().join(interaction_file_name);

        let interaction = load_interaction(&interaction_file_name)
            .map_err(|e| format!("Failed to load interaction: {}", e))?;

        let old_system = compute_spec(old_exo).await;
        let new_system = compute_spec(new_exo).await;

        print!("\t\t\t\t{}:", interaction_name);

        let migration = if kind == TestKind::Up {
            migrate_interactively(old_system, new_system, scope, &interaction).await
        } else {
            migrate_interactively(new_system, old_system, scope, &interaction).await
        }
        .map_err(|e| format!("Failed to migrate: {} {}", interaction_name, e))?;

        let expected_file_path = subfolder.join(format!("{}.sql", interaction_name));

        let expected_migration = std::fs::read_to_string(&expected_file_path).unwrap_or_default();

        assert_sql_eq(
            migration,
            &expected_migration,
            subfolder.clone(),
            &interaction_name,
        )
        .map_err(|e| format!("Failed to assert SQL: {}", e))?;

        println!("{}", "pass".green());
    }

    Ok(())
}

fn assert_sql_eq<P: AsRef<Path>>(
    actual: Migration,
    expected: &str,
    folder: P,
    migration_file_prefix: &str,
) -> Result<(), String> {
    {
        // Check if strings match. This lets us avoid parsing the SQL (which in some cases doesn't work with syntax such as pgvector indexes)
        // TODO: Contribute to sqlparser to support pgvector and other cases where parsing fails
        let mut buffer = std::io::Cursor::new(vec![]);
        actual.write(&mut buffer, false).unwrap();
        let actual_sql = String::from_utf8(buffer.into_inner()).unwrap();

        if actual_sql.lines().count() == expected.lines().count()
            && actual_sql
                .lines()
                .zip(expected.lines())
                .all(|(a, e)| a.trim() == e.trim())
        {
            return Ok(());
        }
    }

    let (actual_sql, actual_destructive_indices) = {
        let actual_sql_destructive_indices = actual
            .statements
            .iter()
            .enumerate()
            .filter_map(|(index, stmt)| {
                if stmt.is_destructive {
                    Some(index)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        let actual_destructive_migration = Migration {
            statements: actual
                .statements
                .iter()
                .map(|stmt| MigrationStatement {
                    statement: stmt.statement.clone(),
                    is_destructive: false,
                })
                .collect::<Vec<_>>(),
        };

        let mut buffer = std::io::Cursor::new(vec![]);
        actual_destructive_migration
            .write(&mut buffer, true)
            .unwrap();
        let actual_sql_str = String::from_utf8(buffer.into_inner()).unwrap();
        (actual_sql_str, actual_sql_destructive_indices)
    };

    let (expected_sql, expected_destructive_indices) = {
        let expected_sql = expected.split(";\n").map(|s| s.trim()).collect::<Vec<_>>();
        let expected_sql_destructive_indices = expected_sql
            .iter()
            .enumerate()
            .filter_map(|(index, s)| {
                if s.starts_with("-- ") {
                    Some(index)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let expected_sql_destructive = expected_sql
            .into_iter()
            .map(|stmt| stmt.strip_prefix("-- ").unwrap_or(stmt).to_string())
            .collect::<Vec<_>>()
            .join(";\n");
        (expected_sql_destructive, expected_sql_destructive_indices)
    };

    let message = format!("{} {}", folder.as_ref().display(), migration_file_prefix);

    if let Err(e) = assert_sql_str_eq(&actual_sql, &expected_sql, &message) {
        dump_migration(&actual, folder, migration_file_prefix)
            .map_err(|e| format!("Failed to dump migration: {}", e))?;
        return Err(e);
    }

    if actual_destructive_indices != expected_destructive_indices {
        return Err(format!(
            "{}: Destructive indices are different.\n  Expected: {:?}\n  Actual:   {:?}",
            message, expected_destructive_indices, actual_destructive_indices,
        ));
    }

    Ok(())
}

fn dump_migration<P: AsRef<Path>>(
    migration: &Migration,
    folder: P,
    migration_file_prefix: &str,
) -> Result<(), std::io::Error> {
    let file_name = relative_path(
        folder,
        PathBuf::from(&format!("{migration_file_prefix}.actual.sql")),
    );
    let mut file = std::fs::File::create(file_name)?;
    migration.write(&mut file, false)?;
    Ok(())
}

fn assert_sql_str_eq(actual: &str, expected: &str, message: &str) -> Result<(), String> {
    // Line-ending insensitive comparison (for Windows compatibility)
    if actual.lines().count() == expected.lines().count()
        && (actual.lines().zip(expected.lines())).all(|(a, e)| a.trim() == e.trim())
    {
        return Ok(());
    }

    let dialect = PostgreSqlDialect {};
    let actual_sql = Parser::parse_sql(&dialect, actual)
        .map_err(|e| format!("Failed to parse actual SQL: {}", e))?;
    let expected_sql = Parser::parse_sql(&dialect, expected)
        .map_err(|e| format!("Failed to parse expected SQL: {}", e))?;

    if actual_sql != expected_sql {
        if actual_sql.len() != expected_sql.len() {
            return Err(format!(
                "{}: Actual SQL length {} is different from expected SQL length {}",
                message,
                actual_sql.len(),
                expected_sql.len(),
            ));
        }

        let mut messages = vec![];

        actual_sql
            .iter()
            .zip(expected_sql.iter())
            .enumerate()
            .for_each(|(i, (a, e))| {
                if a != e {
                    messages.push(format!(
                        "{}: Statement at index {} is different.\n  Expected: {}\n  Actual:   {}",
                        message, i, e, a
                    ));
                }
            });

        if !messages.is_empty() {
            return Err(messages.join("\n"));
        }
    }

    Ok(())
}

fn base_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/migration-test-data")
}

fn relative_path<P1: AsRef<Path>, P2: AsRef<Path>>(folder: P1, path: P2) -> PathBuf {
    let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/migration-test-data");

    base_path.join(folder).join(path)
}

fn read_relative_file<P1: AsRef<Path>, P2: AsRef<Path>>(
    folder: P1,
    path: P2,
) -> Result<String, std::io::Error> {
    std::fs::read_to_string(relative_path(folder, path))
}

async fn create_postgres_system_from_str(
    model_str: &str,
    file_name: String,
) -> Result<PostgresCoreSubsystem, ModelSerializationError> {
    let system = builder::build_system_from_str(
        model_str,
        file_name,
        vec![Box::new(
            postgres_builder::PostgresSubsystemBuilder::default(),
        )],
        BuildMode::Build,
    )
    .await
    .unwrap();

    deserialize_postgres_subsystem(system)
}

fn deserialize_postgres_subsystem(
    system: SerializableSystem,
) -> Result<PostgresCoreSubsystem, ModelSerializationError> {
    let postgres_subsystem = system
        .subsystems
        .into_iter()
        .find(|subsystem| subsystem.id == "postgres");

    match postgres_subsystem {
        Some(subsystem) => {
            let postgres_core_subsystem = PostgresCoreSubsystem::deserialize(subsystem.core.0)?;
            Ok(postgres_core_subsystem)
        }
        None => Ok(PostgresCoreSubsystem::default()),
    }
}

async fn compute_spec(model: &str) -> DatabaseSpec {
    let postgres_core_subsystem = create_postgres_system_from_str(model, "test.exo".to_string())
        .await
        .unwrap();

    DatabaseSpec::from_database(&postgres_core_subsystem.database)
}

fn load_interaction(file_name: &PathBuf) -> Result<PredefinedMigrationInteraction, String> {
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
            table_actions.push(TableAction::Rename(
                string_to_table_name(&rename_table.old_table),
                string_to_table_name(&rename_table.new_table),
            ));
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
