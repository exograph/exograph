use anyhow::{Context, Result};
use futures::FutureExt;
use std::{
    path::Path,
    sync::{mpsc::Sender, Arc},
    time::Duration,
};

use exo_sql::testing::db::EphemeralDatabaseServer;

use crate::{execution::run_introspection_test, model::TestSuite};

use super::{build_exo_ir_file, TestResult};

impl TestSuite {
    pub fn run(
        self,
        run_introspection_tests: bool,
        ephemeral_server: Arc<Box<dyn EphemeralDatabaseServer + Send + Sync>>,
        tx: Sender<Result<TestResult>>,
        tasks: crossbeam_channel::Sender<Box<dyn FnOnce() + Send>>,
    ) {
        let TestSuite { project_dir, tests } = self;

        let project_dir = project_dir.clone();
        let tx = tx.clone();
        let ephemeral_server = ephemeral_server.clone();

        tasks
            .send(Box::new(move || match build_exo_ir_file(&project_dir) {
                Ok(()) => {
                    let runtime = tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(2)
                        .enable_all()
                        .build()
                        .unwrap();
                    let local = tokio::task::LocalSet::new();
                    local.block_on(&runtime, async move {
                        fn report_panic(model_path: &Path) -> Result<TestResult> {
                            Err(anyhow::anyhow!("Panic during test run: {}", model_path.display()))
                        }

                        if run_introspection_tests {
                            let result =
                                std::panic::AssertUnwindSafe(run_introspection_test(&project_dir))
                                    .catch_unwind()
                                    .await;
                            tx.send(result.unwrap_or_else(|_| report_panic(&project_dir)))
                                .map_err(|_| ())
                                .unwrap();
                        };

                        for test in tests.iter() {
                            let mut retries = test.retries;
                            let mut pause = 1000;
                            loop {
                                let result = std::panic::AssertUnwindSafe(test.run(
                                    &project_dir,
                                    ephemeral_server.as_ref().as_ref()
                                        as &dyn EphemeralDatabaseServer,
                                ))
                                .catch_unwind()
                                .await;

                                if result.is_err() {
                                    // Don't retry after a panic
                                    retries = 0;
                                }

                                let result = result.unwrap_or_else(|_| report_panic(&project_dir));
                                let test_succeeded = result.as_ref().map(|t| t.is_success()).unwrap_or_else(|_| false);

                                if retries == 0 || test_succeeded {
                                    tx.send(result).map_err(|_| ()).unwrap();
                                    break;
                                }
                                println!("Test with configured retries failed. Waiting for {pause} ms before retrying");
                                tokio::time::sleep(Duration::from_millis(pause)).await;
                                pause *= 2;
                                retries -= 1;
                            }
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
}
