use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use colored::Colorize;
use exo_sql_model::transformer::SelectTransformer;
use exo_sql_pg::Postgres;
use exo_sql_pg::{Database, ExpressionBuilder};
use exo_sql_pg_schema::DatabaseSpec;
use exo_sql_pg_schema::MigrationScopeMatches;
use exo_sql_pg_schema::WithIssues;
use regex::Regex;
use wildmatch::WildMatch;

use crate::assertion::compare_results;
use crate::query_parser;
use crate::test_file::SqlTestFile;

struct TestResult {
    name: String,
    outcome: TestOutcome,
}

enum TestOutcome {
    Pass,
    Fail(String),
}

/// Run all test fixtures found under `root_directory`.
///
/// Discovers fixture directories (those containing `schema.sql`), then runs
/// all `.sqltest` files in each, optionally filtered by `pattern`.
///
/// Returns `Ok(())` if all tests pass, `Err` if any fail.
pub async fn run(root_directory: &Path, pattern: &Option<String>, backend: &str) -> Result<()> {
    let start = Instant::now();
    let fixtures = discover_fixtures(root_directory);

    if fixtures.is_empty() {
        bail!("No test fixtures found under {}", root_directory.display());
    }

    let mut results: Vec<TestResult> = Vec::new();

    for fixture in &fixtures {
        let fixture_name = fixture
            .dir
            .strip_prefix(root_directory)
            .unwrap_or(&fixture.dir)
            .display()
            .to_string();

        let effective_backend = fixture.backend.as_deref().unwrap_or(backend);

        println!(
            "{}",
            format!("* Running tests in {fixture_name}").blue().bold()
        );

        let fixture_results = run_fixture(fixture, effective_backend, pattern).await;
        results.extend(fixture_results);
    }

    let passed = results
        .iter()
        .filter(|r| matches!(r.outcome, TestOutcome::Pass))
        .count();
    let total = results.len();
    let elapsed = start.elapsed().as_secs_f64();

    println!();
    if passed == total {
        println!(
            "{}",
            format!("* Test results: PASS. {passed} passed out of {total} total in {elapsed:.1}s")
                .green()
                .bold()
        );
        Ok(())
    } else {
        println!(
            "{}",
            format!("* Test results: FAIL. {passed} passed out of {total} total in {elapsed:.1}s")
                .red()
                .bold()
        );
        bail!("Test failures")
    }
}

async fn run_fixture(
    fixture: &FixtureInfo,
    backend: &str,
    pattern: &Option<String>,
) -> Vec<TestResult> {
    let fixture_dir = &fixture.dir;
    let schema_sql = std::fs::read_to_string(fixture_dir.join(&fixture.schema_file))
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", fixture.schema_file));
    let init_sqls = load_init_files(fixture_dir, backend);
    let test_files = discover_test_files(&fixture_dir.join("tests"), backend);

    if test_files.is_empty() {
        println!("  {}", "No test files found".yellow());
        return vec![];
    }

    let mut full_init = schema_sql;
    for init_sql in &init_sqls {
        full_init.push('\n');
        full_init.push_str(init_sql);
    }

    let database = Arc::new(introspect_schema(&full_init).await);
    let full_init = Arc::new(full_init);

    let mut join_set = tokio::task::JoinSet::new();

    for (test_path, test_file) in test_files {
        let test_name = test_path
            .strip_prefix(fixture_dir)
            .unwrap_or(&test_path)
            .display()
            .to_string();

        if let Some(pattern) = pattern
            && !WildMatch::new(pattern).matches(&test_name)
        {
            continue;
        }

        let database = Arc::clone(&database);
        let full_init = Arc::clone(&full_init);

        join_set.spawn(async move {
            run_single_test(&test_name, &test_file, &database, &full_init).await
        });
    }

    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        let result = result.expect("Test task panicked");
        match &result.outcome {
            TestOutcome::Pass => {
                println!("  {} {}", "PASS".green(), result.name);
            }
            TestOutcome::Fail(err) => {
                println!("  {} {}", "FAIL".red(), result.name);
                println!("       {}", err.red());
            }
        }
        results.push(result);
    }

    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

async fn introspect_schema(init_script: &str) -> Database {
    exo_sql_pg_connect::testing::with_init_script(init_script, |client| async move {
        let WithIssues {
            value: database_spec,
            ..
        } = DatabaseSpec::from_live_database(&client, &MigrationScopeMatches::all_schemas())
            .await
            .expect("Failed to introspect database schema");

        database_spec.to_database()
    })
    .await
}

async fn run_single_test(
    test_name: &str,
    test_file: &SqlTestFile,
    database: &Database,
    init_script: &str,
) -> TestResult {
    let result: Result<()> = async {
        let abstract_select = query_parser::parse_query(
            &test_file.query.statement,
            &test_file.query.params,
            database,
        )
        .map_err(|e| anyhow::anyhow!("Parse error: {e}"))?;

        let json_aggregate = matches!(abstract_select.selection, exo_sql_pg::Selection::Json(..));

        let select = Postgres {}.to_select(abstract_select, database);
        let (sql, params) = select.to_sql(database);

        let params_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params.iter().map(|p| p.param.as_pg()).collect();

        exo_sql_pg_connect::testing::with_init_script(init_script, |client| async move {
            let rows = client
                .query(&sql, &params_refs)
                .await
                .with_context(|| format!("[{test_name}] Query execution failed\nSQL: {sql}"))?;

            compare_results(
                test_name,
                &rows,
                &test_file.expect.result,
                &test_file.expect.unordered_paths,
                json_aggregate,
            )
        })
        .await
    }
    .await;

    let outcome = match result {
        Ok(()) => TestOutcome::Pass,
        Err(e) => TestOutcome::Fail(format!("{e:#}")),
    };

    TestResult {
        name: test_name.to_string(),
        outcome,
    }
}

struct FixtureInfo {
    dir: PathBuf,
    schema_file: String,
    backend: Option<String>,
}

fn discover_fixtures(root: &Path) -> Vec<FixtureInfo> {
    let mut fixtures = Vec::new();

    if let Some((schema_file, backend)) = detect_schema_file(root) {
        fixtures.push(FixtureInfo {
            dir: root.to_path_buf(),
            schema_file,
            backend,
        });
    }

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                fixtures.extend(discover_fixtures(&path));
            }
        }
    }

    fixtures.sort_by(|a, b| a.dir.cmp(&b.dir));
    fixtures
}

/// Detect a schema file in `dir`, returning `(filename, optional backend)`.
/// E.g. `schema.sql` → `("schema.sql", None)`, `schema.pg.sql` → `("schema.pg.sql", Some("pg"))`.
fn detect_schema_file(dir: &Path) -> Option<(String, Option<String>)> {
    static SCHEMA_FILE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^schema(?:\.(\w+))?\.sql$").unwrap());

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(caps) = SCHEMA_FILE_RE.captures(&name) {
                let backend = caps.get(1).map(|m| m.as_str().to_string());
                return Some((name, backend));
            }
        }
    }
    None
}

fn load_init_files(dir: &Path, backend: &str) -> Vec<String> {
    let mut init_files: Vec<PathBuf> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();

            let is_init = name.starts_with("init") && name.ends_with(".sql");
            let is_backend_specific = is_init && name.ends_with(&format!(".{backend}.sql"));

            if is_backend_specific {
                init_files.push(path);
            } else if is_init && !is_backend_specific {
                let override_name = name.replace(".sql", &format!(".{backend}.sql"));
                let override_path = dir.join(&override_name);
                if !override_path.exists() {
                    init_files.push(path);
                }
            }
        }
    }

    init_files.sort();
    init_files
        .iter()
        .map(|p| {
            std::fs::read_to_string(p)
                .unwrap_or_else(|e| panic!("Failed to read {}: {e}", p.display()))
        })
        .collect()
}

fn discover_test_files(tests_dir: &Path, backend: &str) -> Vec<(PathBuf, SqlTestFile)> {
    let mut test_files: Vec<(PathBuf, SqlTestFile)> = Vec::new();

    if !tests_dir.exists() {
        return test_files;
    }

    let mut paths: Vec<PathBuf> = Vec::new();
    collect_test_files(tests_dir, &mut paths, backend);
    paths.sort();

    for path in paths {
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", path.display()));
        let test_file: SqlTestFile = toml::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()));
        test_files.push((path, test_file));
    }

    test_files
}

fn collect_test_files(dir: &Path, paths: &mut Vec<PathBuf>, backend: &str) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_test_files(&path, paths, backend);
            } else {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                let is_generic = name.ends_with(".sqltest")
                    && !name[..name.len() - ".sqltest".len()].contains('.');
                let is_backend_specific = name.ends_with(&format!(".{backend}.sqltest"));

                if is_generic || is_backend_specific {
                    paths.push(path);
                }
            }
        }
    }
}
