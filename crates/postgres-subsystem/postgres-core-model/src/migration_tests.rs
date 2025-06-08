use std::path::{Path, PathBuf};

use crate::subsystem::PostgresCoreSubsystem;
use exo_sql::{
    schema::{
        database_spec::DatabaseSpec,
        migration::{
            Migration, MigrationStatement, PredefinedMigrationInteraction, migrate_interactively,
        },
        spec::{MigrationScope, MigrationScopeMatches},
    },
    testing::test_support,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;

use colored::Colorize;

use core_model_builder::plugin::BuildMode;
use core_plugin_shared::{
    error::ModelSerializationError, serializable_system::SerializableSystem,
    system_serializer::SystemSerializer,
};
#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn all_tests() {
    common::test_support::run_tests(
        env!("CARGO_MANIFEST_DIR"),
        "_EXO_DEV_MIGRATION_TEST_FILTER",
        "src/migration-test-data",
        |test_config_name, test_path| async move { single_test(test_config_name, test_path).await },
    )
    .await
    .unwrap();
}

async fn single_test(folder: String, test_path: PathBuf) -> Result<(), String> {
    println!("Testing {}", folder);
    let old_exo = read_relative_file(&test_path, "old/src/index.exo")
        .map_err(|e| format!("Failed to read old exo: {}", e))?;
    let new_exo = read_relative_file(&test_path, "new/src/index.exo")
        .map_err(|e| format!("Failed to read new exo: {}", e))?;

    let old_system = compute_spec(&old_exo).await;
    let new_system = compute_spec(&new_exo).await;

    let scope_dirs = std::fs::read_dir(&test_path)
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

        let scope_folder = test_path.join(scope_dir_name);

        println!("\tscope {}:", scope_spec_name);

        if let Err(e) = assert_for_scope(&old_system, &new_system, &scope_folder, &scope).await {
            println!("{}: {}", scope_folder.display(), e);
            failed = true;
        }
    }

    if failed {
        Err(format!("{}: Some tests failed", folder))
    } else {
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TestKind {
    Creation(SystemKind),
    Migration(SystemKind, SystemKind),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum SystemKind {
    Old,
    New,
}

impl TestKind {
    fn kind_str(&self) -> &str {
        match self {
            TestKind::Creation(SystemKind::Old) => "old",
            TestKind::Creation(SystemKind::New) => "new",
            TestKind::Migration(SystemKind::Old, SystemKind::New) => "up",
            TestKind::Migration(SystemKind::New, SystemKind::Old) => "down",
            TestKind::Migration(SystemKind::Old, SystemKind::Old) => "idempotent-old",
            TestKind::Migration(SystemKind::New, SystemKind::New) => "idempotent-new",
        }
    }
}

async fn assert_for_scope(
    old_system: &DatabaseSpec,
    new_system: &DatabaseSpec,
    folder: &PathBuf,
    scope: &MigrationScope,
) -> Result<(), String> {
    let old_expected_sql = read_relative_file(folder, "old.sql").unwrap_or_default();
    let new_expected_sql = read_relative_file(folder, "new.sql").unwrap_or_default();
    let up_expected_sql = read_relative_file(folder, "up.sql").unwrap_or_default();
    let down_expected_sql = read_relative_file(folder, "down.sql").unwrap_or_default();

    let mut failed = false;

    if let Err(e) = assert_creation_and_self_migration(
        old_system,
        &old_expected_sql,
        scope,
        folder,
        SystemKind::Old,
    )
    .await
    {
        println!("Old creation failed: {}", e);
        failed = true;
    } else {
        println!("\t\told-creation: {}", "pass".green());
    }

    if let Err(e) = assert_creation_and_self_migration(
        new_system,
        &new_expected_sql,
        scope,
        folder,
        SystemKind::New,
    )
    .await
    {
        println!("New creation failed: {}", e);
        failed = true;
    } else {
        println!("\t\tnew-creation: {}", "pass".green());
    }

    if let Err(e) = assert_migration(
        old_system,
        new_system,
        &up_expected_sql,
        scope,
        folder,
        TestKind::Migration(SystemKind::Old, SystemKind::New),
    )
    .await
    {
        println!("Up failed: {}", e);
        failed = true;
    } else {
        println!("\t\tup: {}", "pass".green());
    }

    if let Err(e) = assert_migration(
        new_system,
        old_system,
        &down_expected_sql,
        scope,
        folder,
        TestKind::Migration(SystemKind::New, SystemKind::Old),
    )
    .await
    {
        println!("Down failed: {}", e);
        failed = true;
    } else {
        println!("\t\tdown: {}", "pass".green());
    }

    if let Err(e) = assert_interactive_migrations(old_system, new_system, scope, folder).await {
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
async fn assert_creation_and_self_migration(
    model_spec: &DatabaseSpec,
    expected: &str,
    migration_scope: &MigrationScope,
    folder: &Path,
    kind: SystemKind,
) -> Result<(), String> {
    let creation = Migration::from_schemas(
        &DatabaseSpec::new(vec![], vec![], vec![]),
        model_spec,
        migration_scope,
    );
    assert_sql_eq(&creation, expected, folder, TestKind::Creation(kind))?;

    let live_migrated_spec = assert_migration_with_live_db(
        &DatabaseSpec::new(vec![], vec![], vec![]),
        model_spec,
        migration_scope,
        &creation,
    )
    .await?;

    for spec in [model_spec, &live_migrated_spec] {
        let self_migration = Migration::from_schemas(spec, spec, migration_scope);
        assert_sql_eq(&self_migration, "", folder, TestKind::Migration(kind, kind))?;
    }

    Ok(())
}

async fn assert_migration(
    old_system: &DatabaseSpec,
    new_system: &DatabaseSpec,
    expected: &str,
    migration_scope: &MigrationScope,
    folder: &Path,
    kind: TestKind,
) -> Result<(), String> {
    let migration = Migration::from_schemas(old_system, new_system, migration_scope);

    assert_sql_eq(&migration, expected, folder, kind)?;
    assert_migration_with_live_db(old_system, new_system, migration_scope, &migration).await?;

    Ok(())
}

async fn assert_migration_with_live_db(
    old_system: &DatabaseSpec,
    new_system: &DatabaseSpec,
    migration_scope: &MigrationScope,
    migration: &Migration,
) -> Result<DatabaseSpec, String> {
    test_support::with_client(move |mut client| async move {
        let creation = Migration::from_schemas(
            &DatabaseSpec::new(vec![], vec![], vec![]),
            old_system,
            migration_scope,
        );

        // If the creation is empty, we had been working with a non-managed schema and can skip the migration
        // TODO: Make this a more robust check
        if creation.statements.is_empty() {
            return Ok(DatabaseSpec::new(vec![], vec![], vec![]));
        }

        creation
            .apply(&mut client, true)
            .await
            .map_err(|e| e.to_string())?;

        migration
            .apply(&mut client, true)
            .await
            .map_err(|e| e.to_string())?;

        let scope_matches = match migration_scope {
            MigrationScope::Specified(scope) => scope,
            MigrationScope::FromNewSpec => {
                &MigrationScopeMatches::from_specs_schemas(&[new_system])
            }
        };

        let live_db_spec = DatabaseSpec::from_live_database(&client, scope_matches)
            .await
            .map_err(|e| format!("Failed to extract live db spec: {}", e))?;

        if live_db_spec.issues.is_empty() {
            Ok(live_db_spec.value)
        } else {
            Err(format!(
                "Live db spec has issues: {:?}",
                live_db_spec.issues
            ))
        }
    })
    .await
}

async fn assert_interactive_migrations<P: AsRef<Path>>(
    old_system: &DatabaseSpec,
    new_system: &DatabaseSpec,
    migration_scope: &MigrationScope,
    folder: P,
) -> Result<(), String> {
    let interactive_dir = folder.as_ref().join("interactive");

    if !std::path::Path::new(&interactive_dir).exists() {
        return Ok(());
    }

    println!("\t\tinteractive:");

    for kind in [
        TestKind::Migration(SystemKind::Old, SystemKind::New),
        TestKind::Migration(SystemKind::New, SystemKind::Old),
    ] {
        assert_interactive_migration(
            old_system,
            new_system,
            kind,
            migration_scope,
            &interactive_dir,
        )
        .await?
    }

    Ok(())
}

async fn assert_interactive_migration(
    old_system: &DatabaseSpec,
    new_system: &DatabaseSpec,
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

        let interaction = PredefinedMigrationInteraction::from_file(&interaction_file_name)
            .map_err(|e| format!("Failed to load interaction: {}", e))?;

        print!("\t\t\t\t{}:", interaction_name);

        let migration = if kind.kind_str() == "up" {
            migrate_interactively(old_system.clone(), new_system.clone(), scope, &interaction).await
        } else {
            migrate_interactively(new_system.clone(), old_system.clone(), scope, &interaction).await
        }
        .map_err(|e| format!("Failed to migrate: {} {}", interaction_name, e))?;

        let expected_file_path = subfolder.join(format!("{}.sql", interaction_name));

        let expected_migration = std::fs::read_to_string(&expected_file_path).unwrap_or_default();

        assert_sql_eq(&migration, &expected_migration, &subfolder, kind)
            .map_err(|e| format!("Failed to assert SQL: {}", e))?;

        assert_migration_with_live_db(old_system, new_system, scope, &migration).await?;

        println!("{}", "pass".green());
    }

    Ok(())
}

fn assert_sql_eq(
    actual: &Migration,
    expected: &str,
    folder: &Path,
    kind: TestKind,
) -> Result<(), String> {
    let remove_previous_file = || {
        let previous_file_path = dump_migration_path(folder, kind).unwrap();
        if previous_file_path.exists() {
            std::fs::remove_file(previous_file_path).unwrap();
        }
    };

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
            remove_previous_file();
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

    let message = format!("{} {}", folder.display(), kind.kind_str());

    if let Err(e) = assert_sql_str_eq(&actual_sql, &expected_sql, &message) {
        dump_migration(actual, folder, kind)
            .map_err(|e| format!("Failed to dump migration: {}", e))?;
        return Err(e);
    } else {
        remove_previous_file();
    }

    if actual_destructive_indices != expected_destructive_indices {
        return Err(format!(
            "{}: Destructive indices are different.\n  Expected: {:?}\n  Actual:   {:?}",
            message, expected_destructive_indices, actual_destructive_indices,
        ));
    }

    Ok(())
}

fn dump_migration(
    migration: &Migration,
    folder: &Path,
    kind: TestKind,
) -> Result<(), std::io::Error> {
    let path = dump_migration_path(folder, kind)?;
    let mut file = std::fs::File::create(path)?;

    migration.write(&mut file, false)?;
    Ok(())
}

fn dump_migration_path(folder: &Path, kind: TestKind) -> Result<PathBuf, std::io::Error> {
    let kind_str = kind.kind_str();

    let path = folder.join(format!("{}.actual.sql", kind_str));
    Ok(path)
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
