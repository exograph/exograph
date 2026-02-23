use anyhow::{Context, Result, bail};
use futures::FutureExt;
use std::{
    ffi::OsStr,
    io::Write,
    path::Path,
    process::Command,
    sync::{Arc, mpsc::Sender},
    thread,
    time::Duration,
};

use exo_sql::testing::db::EphemeralDatabaseServer;

use super::introspection_tests::run_introspection_test;
use crate::model::TestSuite;

use super::TestResult;

impl TestSuite {
    pub fn run(
        self,
        run_introspection_tests: bool,
        generate_rpc_expected: bool,
        ephemeral_server: Arc<Box<dyn EphemeralDatabaseServer + Send + Sync>>,
        tx: Sender<Result<TestResult>>,
        tasks: crossbeam_channel::Sender<Box<dyn FnOnce() + Send>>,
    ) {
        let project_dir = self.project_dir.clone();
        let tx = tx.clone();
        let ephemeral_server = ephemeral_server.clone();

        tasks
            .send(Box::new(move || match self.build_exo_ir_file() {
                Ok(()) => {
                    let runtime = tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .unwrap();
                    let local = tokio::task::LocalSet::new();
                    local.block_on(&runtime, async move {
                        fn report_panic(model_path: &Path) -> Result<TestResult> {
                            Err(anyhow::anyhow!(
                                "Panic during test run: {}",
                                model_path.display()
                            ))
                        }

                        if run_introspection_tests {
                            let result = std::panic::AssertUnwindSafe(run_introspection_test(
                                &project_dir,
                                generate_rpc_expected,
                            ))
                            .catch_unwind()
                            .await;
                            tx.send(result.unwrap_or_else(|_| report_panic(&project_dir)))
                                .map_err(|_| ())
                                .unwrap();
                        };

                        for test in self.tests.iter() {
                            test.run(
                                &project_dir,
                                ephemeral_server.as_ref().as_ref() as &dyn EphemeralDatabaseServer,
                                tx.clone(),
                            )
                            .await;
                        }
                    })
                }
                Err(e) => tx
                    .send(Err(e).with_context(|| {
                        format!(
                            "While trying to build exo_ir file for {}",
                            project_dir.display()
                        )
                    }))
                    .map_err(|_| ())
                    .unwrap(),
            }))
            .unwrap();
    }

    fn build_exo_ir_file(&self) -> Result<()> {
        self.build_prerequisites()?;

        // Use std::env::current_exe() so that we run the same "exo" that invoked us (specifically, avoid using another exo on $PATH)
        // Retry the build to handle transient failures (e.g. file locking races on Windows
        // when multiple parallel builds compete for shared caches like Deno's esbuild binary,
        // or brief network unavailability)
        let max_attempts = 3;
        for attempt in 0..max_attempts {
            match run_command(
                std::env::current_exe()?.as_os_str().to_str().unwrap(),
                [OsStr::new("build")],
                Some(self.project_dir.as_ref()),
                "Could not build the exo_ir.",
            ) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempt >= max_attempts - 1 {
                        return Err(e);
                    }

                    let pause = Duration::from_millis(1000 * 2u64.pow(attempt as u32));
                    eprintln!(
                        "Build failed for {} (attempt {}/{}): {:#}. Retrying in {:?}...",
                        self.project_dir.display(),
                        attempt + 1,
                        max_attempts,
                        e,
                        pause
                    );
                    thread::sleep(pause);
                }
            }
        }
        unreachable!()
    }

    // Run all scripts of the "build*.sh" form in the same directory as the model
    fn build_prerequisites(&self) -> Result<()> {
        let mut build_files = vec![];

        for dir_entry in self.project_dir.join("tests").read_dir()? {
            let dir_entry = dir_entry?;
            let path = dir_entry.path();

            if path.is_file() {
                let file_name = path.file_name().unwrap().to_str().unwrap();
                if file_name.starts_with("build") && path.extension().unwrap() == "sh" {
                    build_files.push(path);
                }
            }
        }

        build_files.sort();

        for build_file in build_files {
            run_command(
                "sh",
                vec![build_file.to_str().unwrap()],
                None,
                &format!("Build script at {} failed to run", build_file.display()),
            )?
        }

        Ok(())
    }
}

// Helper to run a command and return an error if it fails
fn run_command<I, S>(
    program: &str,
    args: I,
    current_dir: Option<&Path>,
    failure_message: &str,
) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(program);
    command.args(args);
    command.env("EXO_SKIP_UPDATE_CHECK", "true");
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    let build_child = command.output()?;

    if !build_child.status.success() {
        std::io::stdout().write_all(&build_child.stdout).unwrap();
        std::io::stderr().write_all(&build_child.stderr).unwrap();
        bail!(failure_message.to_string());
    }

    Ok(())
}
