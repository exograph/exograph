use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::serde_json;
use deno_core::JsRuntime;
use std::path::PathBuf;
use tracing::error;
use tracing::instrument;
use tracing::log::debug;

use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use serde_json::Value;

use std::collections::HashMap;
use std::fs;
use std::rc::Rc;
use std::sync::Arc;

use std::convert::TryFrom;

use anyhow::{anyhow, Result};

use deno_core::v8;

mod embedded_module_loader;
use self::embedded_module_loader::EmbeddedModuleLoader;

fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

const JSERROR_PREFIX: &str = "Uncaught ClaytipError: ";

pub enum UserCode {
    LoadFromMemory { script: String, path: String },
    LoadFromFs(PathBuf),
}

pub struct DenoModule {
    worker: MainWorker,
    shim_object_names: Vec<String>,
    user_code: UserCode,
}

/// Set of shared resources between DenoModules.
/// Cloning one DenoModuleSharedState and providing it to a set of DenoModules will
/// give them all access to the state through Arc<>s!
#[derive(Clone, Default)]
pub struct DenoModuleSharedState {
    pub blob_store: BlobStore,
    pub broadcast_channel: InMemoryBroadcastChannel,
    // TODO
    //  shared_array_buffer_store
    //  compiled_wasm_module_store
}

/// A Deno-based runner for JavaScript.
///
/// DenoModule has no concept of Claytip; it exists solely to configure the JavaScript execution environment
/// and to load & execute methods in the Deno runtime from sources.
impl DenoModule {
    pub async fn new<F>(
        user_code: UserCode,
        user_agent_name: &str,
        shims: &[(&str, &str)],
        register_ops: F,
        shared_state: DenoModuleSharedState,
    ) -> Result<Self, AnyError>
    where
        F: FnOnce(&mut JsRuntime) + Send + Sync,
    {
        let shim_source_code = {
            let shims_source_codes: Vec<_> = shims
                .iter()
                .map(|(shim_name, source)| format!("globalThis.{} = {};", shim_name, source))
                .collect();

            shims_source_codes.join("\n")
        };

        let user_module_path = match &user_code {
            UserCode::LoadFromFs(user_module_path) => fs::canonicalize(user_module_path)?
                .to_string_lossy()
                .to_string(),
            UserCode::LoadFromMemory { path, .. } => format!("file:///__claytip/{}", path),
        };

        let source_code = format!(
            "import * as mod from '{}'; globalThis.mod = mod; {}",
            &user_module_path, shim_source_code
        );

        let main_module_specifier = "file:///main.js".to_string();
        let module_loader = Rc::new(EmbeddedModuleLoader {
            source_code_map: match &user_code {
                UserCode::LoadFromFs(_) => vec![("file:///main.js".to_owned(), source_code)],
                UserCode::LoadFromMemory { path, script } => vec![
                    ("file:///main.js".to_owned(), source_code),
                    (format!("file:///__claytip/{}", path), script.to_string()),
                ],
            }
            .into_iter()
            .collect(),
        });

        let create_web_worker_cb = Arc::new(|_| {
            todo!("Web workers are not supported in the example");
        });

        let options = WorkerOptions {
            bootstrap: BootstrapOptions {
                apply_source_maps: false,
                args: vec![],
                cpu_count: 1,
                debug_flag: false,
                enable_testing_features: false,
                location: None,
                no_color: false,
                runtime_version: "x".to_string(),
                ts_version: "x".to_string(),
                unstable: true,
            },
            extensions: vec![],
            unsafely_ignore_certificate_errors: None,
            root_cert_store: None,
            user_agent: user_agent_name.to_string(),
            seed: None,
            js_error_create_fn: None,
            create_web_worker_cb,
            maybe_inspector_server: None,
            should_break_on_first_statement: false,
            module_loader,
            get_error_class_fn: Some(&get_error_class_name),
            origin_storage_dir: None,
            blob_store: shared_state.blob_store,
            broadcast_channel: shared_state.broadcast_channel,
            shared_array_buffer_store: None,
            compiled_wasm_module_store: None,
        };

        let main_module = deno_core::resolve_url(&main_module_specifier)?;
        let permissions = Permissions::allow_all();

        let mut worker =
            MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);

        register_ops(&mut worker.js_runtime);
        worker.js_runtime.sync_ops_cache();

        worker.execute_main_module(&main_module).await?;
        worker
            .execute_script("", include_str!("./utils.js"))
            .unwrap();
        worker.run_event_loop(false).await?;

        let shim_object_names = shims.iter().map(|(name, _)| name.to_string()).collect();

        let deno_module = Self {
            worker,
            shim_object_names,
            user_code,
        };

        Ok(deno_module)
    }

    #[instrument(
        name = "deno_module::execute_function"
        level = "debug"
        skip_all
        )]
    pub async fn execute_function(&mut self, function_name: &str, args: Vec<Arg>) -> Result<Value> {
        debug!("starting function execution of {}", function_name);

        let worker = &mut self.worker;
        let runtime = &mut worker.js_runtime;

        let func_value_string = format!("mod.{}", function_name);

        let func_value = runtime.execute_script("", &func_value_string)?;

        let shim_objects_vals: Vec<_> = self
            .shim_object_names
            .iter()
            .map(|name| runtime.execute_script("", name))
            .collect::<Result<_, _>>()?;

        let shim_objects: HashMap<_, _> = self
            .shim_object_names
            .iter()
            .zip(shim_objects_vals.into_iter())
            .collect();

        let global = {
            let scope = &mut runtime.handle_scope();

            let mut tc_scope = v8::TryCatch::new(scope);
            let tc_scope_ref = &mut tc_scope;

            let args: Vec<_> = args
                .into_iter()
                .map(|v| match v {
                    Arg::Serde(v) => Ok(serde_v8::to_v8(tc_scope_ref, v)?),
                    Arg::Shim(name) => Ok(shim_objects
                        .get(&name)
                        .ok_or_else(|| anyhow!("Missing shim {}", &name))?
                        .open(tc_scope_ref)
                        .to_object(tc_scope_ref)
                        .unwrap()
                        .into()),
                })
                .collect::<Result<Vec<_>, AnyError>>()?;

            let func_obj = func_value
                .open(tc_scope_ref)
                .to_object(tc_scope_ref)
                .ok_or_else(|| {
                    anyhow!(
                        "no function named {} exported from {}",
                        function_name,
                        match &self.user_code {
                            UserCode::LoadFromMemory { path, .. } => path,
                            UserCode::LoadFromFs(path) => path.to_str().unwrap(),
                        }
                    )
                })?;
            let func = v8::Local::<v8::Function>::try_from(func_obj)?;

            let undefined = v8::undefined(tc_scope_ref);
            let local = func.call(tc_scope_ref, undefined.into(), &args);

            let local = match local {
                Some(value) => value,
                None => {
                    let exception = tc_scope_ref.exception().unwrap();
                    let js_error = JsError::from_v8_exception(tc_scope_ref, exception);

                    error!(%js_error, "Exception executing function");

                    if js_error.message.starts_with(JSERROR_PREFIX) {
                        // code threw an explicit Error(...), expose to user
                        let message = js_error.message.strip_prefix(JSERROR_PREFIX).unwrap();
                        return Err(anyhow!(message.to_owned()));
                    } else {
                        // generic error message
                        return Err(anyhow!("Internal server error"));
                    }
                }
            };

            v8::Global::new(tc_scope_ref, local)
        };

        {
            let value = runtime.resolve_value(global).await.map_err(|err| {
                // got some AnyError from Deno internals...
                error!(%err);
                anyhow!("Internal server error")
            })?;

            let scope = &mut runtime.handle_scope();
            let res = v8::Local::new(scope, value);
            let res: Value = serde_v8::from_v8(scope, res)?;
            Ok(res)
        }
    }

    /// Put a single instance of a type into Deno's op_state
    pub fn put<T: 'static>(&mut self, val: T) {
        self.worker.js_runtime.op_state().borrow_mut().put(val)
    }
}

#[derive(Debug, Clone)]
pub enum Arg {
    Serde(serde_json::Value),
    Shim(String),
}
