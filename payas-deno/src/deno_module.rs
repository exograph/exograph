use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::serde_json;
use deno_core::v8;
use deno_core::Extension;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::ops::io::Stdio;
use deno_runtime::permissions::Permissions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;

use std::path::PathBuf;
use tracing::error;
use tracing::instrument;

use serde_json::Value;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::rc::Rc;
use std::sync::Arc;

use super::embedded_module_loader::EmbeddedModuleLoader;
use anyhow::{anyhow, Result};

fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

pub enum UserCode {
    LoadFromMemory { script: String, path: String },
    LoadFromFs(PathBuf),
}

pub struct DenoModule {
    worker: MainWorker,
    shim_object_names: Vec<String>,
    user_code: UserCode,
    explicit_error_class_name: Option<&'static str>,
}

/// A Deno-based runner for JavaScript.
///
/// DenoModule has no concept of Claytip; it exists solely to configure the JavaScript execution environment
/// and to load & execute methods in the Deno runtime from sources.
///
/// # Arguments
/// * `user_code` - The user code with exported functions (which may then be invoked using `DenoModule.execute_function` ).
/// * `user_agent_name` - The name of the user agent
/// * `shims` - A list of shims to load (each tuple is the name of the shim and the source code).
/// * `additional_code` - Any additional code (such as definition of a global type or a global function) to load.
/// * `extensions` - A list of extensions to load.
/// * `shared_state` - A shared state object to pass to the worker.
/// * `explicit_error_class_name` - The name of the class whose message will be used to report errors.
impl DenoModule {
    pub async fn new(
        user_code: UserCode,
        user_agent_name: &str,
        shims: Vec<(&str, &str)>,
        additional_code: Vec<&str>,
        extensions: Vec<Extension>,
        shared_state: DenoModuleSharedState,
        explicit_error_class_name: Option<&'static str>,
    ) -> Result<Self, AnyError> {
        let shim_source_code = {
            let shims_source_codes: Vec<_> = shims
                .iter()
                .map(|(shim_name, source)| format!("globalThis.{shim_name} = {source};"))
                .collect();

            shims_source_codes.join("\n")
        };

        let user_module_path = match &user_code {
            UserCode::LoadFromFs(user_module_path) => fs::canonicalize(user_module_path)?
                .to_string_lossy()
                .to_string(),
            UserCode::LoadFromMemory { path, .. } => format!("file:///{path}"),
        };

        let source_code = format!(
            "import * as mod from '{user_module_path}'; globalThis.mod = mod; {shim_source_code}"
        );

        let main_module_specifier = "file:///main.js".to_string();
        let module_loader = Rc::new(EmbeddedModuleLoader {
            source_code_map: match &user_code {
                UserCode::LoadFromFs(_) => vec![("file:///main.js".to_owned(), source_code)],
                UserCode::LoadFromMemory { path, script } => vec![
                    ("file:///main.js".to_owned(), source_code),
                    (format!("file:///{path}"), script.to_string()),
                ],
            }
            .into_iter()
            .collect(),
        });

        let create_web_worker_cb = Arc::new(|_| {
            todo!("Web workers are not supported");
        });

        let options = WorkerOptions {
            bootstrap: BootstrapOptions {
                args: vec![],
                cpu_count: 1,
                debug_flag: false,
                enable_testing_features: false,
                location: None,
                no_color: false,
                runtime_version: "x".to_string(),
                ts_version: "x".to_string(),
                unstable: true,
                is_tty: false,
                user_agent: user_agent_name.to_string(),
            },
            extensions,
            unsafely_ignore_certificate_errors: None,
            root_cert_store: None,
            seed: None,
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
            web_worker_preload_module_cb: Arc::new(|_| todo!()),
            source_map_getter: None,
            format_js_error_fn: None,
            stdio: Stdio::default(),
        };

        let main_module = deno_core::resolve_url(&main_module_specifier)?;
        let permissions = Permissions::allow_all();

        let mut worker =
            MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);

        worker.execute_main_module(&main_module).await?;

        additional_code.iter().for_each(|code| {
            worker.execute_script("", code).unwrap();
        });

        worker.run_event_loop(false).await?;

        let shim_object_names = shims.iter().map(|(name, _)| name.to_string()).collect();

        let deno_module = Self {
            worker,
            shim_object_names,
            user_code,
            explicit_error_class_name,
        };

        Ok(deno_module)
    }

    /// Execute a function in the Deno runtime.
    /// # Arguments
    /// * `function_name` - The name of the function to execute (must be exported by the user code).
    /// * `args` - The arguments to pass to the function.
    /// # Returns
    /// * `Result<Value, AnyError>` - The result of the function call.
    #[instrument(
        name = "deno_module::execute_function"
        level = "debug"
        skip_all
        )]
    pub async fn execute_function(&mut self, function_name: &str, args: Vec<Arg>) -> Result<Value> {
        let worker = &mut self.worker;
        let runtime = &mut worker.js_runtime;

        let func_value_string = format!("mod.{function_name}");

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

                    match self.explicit_error_class_name {
                        Some(explicit_error_class_name)
                            if js_error.name.as_ref().unwrap_or(&("".to_string()))
                                == explicit_error_class_name =>
                        {
                            // code threw an explicit Error(...), expose to user
                            let message = js_error
                                .message
                                .unwrap_or_else(|| "Unknown error".to_string());
                            return Err(anyhow!(message));
                        }
                        _ => {
                            // generic error message
                            return Err(anyhow!("Internal server error"));
                        }
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
    pub fn put<T: 'static>(&mut self, val: T) -> Result<()> {
        self.worker.js_runtime.op_state().try_borrow_mut()?.put(val);
        Ok(())
    }

    /// Try to take a single instance of a type from Deno's op_state
    pub fn take<T: 'static>(&mut self) -> Result<Option<T>> {
        Ok(self
            .worker
            .js_runtime
            .op_state()
            .try_borrow_mut()?
            .try_take())
    }
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

/// Argument to a DenoModule function.
#[derive(Debug, Clone)]
pub enum Arg {
    /// A normal value that can be serialized to/from v8.
    Serde(serde_json::Value),
    /// Name of the shim to be used. The string used must be one of the names
    /// (the first part of each tuple) provided to the `shims` argument to the
    /// `DenoModule::new` function.
    Shim(String),
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use deno_core::op;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn test_direct_sync() {
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(Path::new("src/test_js/direct.js").to_owned()),
            "deno_module",
            vec![],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
            None,
        )
        .await
        .unwrap();

        let sync_ret_value = deno_module
            .execute_function(
                "addAndDouble",
                vec![
                    Arg::Serde(Value::Number(4.into())),
                    Arg::Serde(Value::Number(2.into())),
                ],
            )
            .await
            .unwrap();

        assert_eq!(sync_ret_value, Value::Number(12.into()));
    }

    #[tokio::test]
    async fn test_direct_async() {
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(Path::new("src/test_js/direct.js").to_owned()),
            "deno_module",
            vec![],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
            None,
        )
        .await
        .unwrap();

        let async_ret_value = deno_module
            .execute_function("getJson", vec![Arg::Serde(Value::String("4".into()))])
            .await
            .unwrap();
        assert_eq!(
            async_ret_value,
            json!({ "userId": 1, "id": 4, "title": "et porro tempora", "completed": true })
        );

        // The JS side doesn't care if the id is a string or a number, so let's use number here
        let async_ret_value = deno_module
            .execute_function("getJson", vec![Arg::Serde(Value::Number(5.into()))])
            .await
            .unwrap();
        assert_eq!(
            async_ret_value,
            json!({ "userId": 1, "id": 5, "title": "laboriosam mollitia et enim quasi adipisci quia provident illum", "completed": false })
        );
    }

    #[tokio::test]
    async fn test_shim_sync() {
        static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("./test_js/shim.js"));

        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(Path::new("src/test_js/through_shim.js").to_owned()),
            "deno_module",
            vec![GET_JSON_SHIM],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
            None,
        )
        .await
        .unwrap();

        let sync_ret_value = deno_module
            .execute_function(
                "addAndDoubleThroughShim",
                vec![
                    Arg::Serde(Value::Number(4.into())),
                    Arg::Serde(Value::Number(5.into())),
                    Arg::Shim("__shim".to_string()),
                ],
            )
            .await
            .unwrap();
        assert_eq!(sync_ret_value, Value::Number(18.into()));

        let sync_ret_value = deno_module
            .execute_function(
                "addAndDoubleThroughShim",
                vec![
                    Arg::Serde(Value::Number(42.into())),
                    Arg::Serde(Value::Number(21.into())),
                    Arg::Shim("__shim".to_string()),
                ],
            )
            .await
            .unwrap();
        assert_eq!(sync_ret_value, Value::Number(126.into()));
    }

    #[tokio::test]
    async fn test_shim_async() {
        static GET_JSON_SHIM: (&str, &str) = ("__shim", include_str!("./test_js/shim.js"));

        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(Path::new("src/test_js/through_shim.js").to_owned()),
            "deno_module",
            vec![GET_JSON_SHIM],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
            None,
        )
        .await
        .unwrap();

        let async_ret_value = deno_module
            .execute_function(
                "getJsonThroughShim",
                vec![
                    Arg::Serde(Value::String("4".into())),
                    Arg::Shim("__shim".to_string()),
                ],
            )
            .await
            .unwrap();
        assert_eq!(
            async_ret_value,
            json!({ "userId": 1, "id": 4, "title": "et porro tempora", "completed": true })
        );

        // The JS side doesn't care if the id is a string or a number, so let's use number here
        let async_ret_value = deno_module
            .execute_function(
                "getJsonThroughShim",
                vec![
                    Arg::Serde(Value::Number(5.into())),
                    Arg::Shim("__shim".to_string()),
                ],
            )
            .await
            .unwrap();
        assert_eq!(
            async_ret_value,
            json!({ "userId": 1, "id": 5, "title": "laboriosam mollitia et enim quasi adipisci quia provident illum", "completed": false })
        );
    }

    #[op]
    fn rust_impl(arg: String) -> Result<String, AnyError> {
        Ok(format!("Register Op: {}", arg))
    }

    #[tokio::test]
    async fn test_register_ops() {
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(Path::new("src/test_js/through_rust_fn.js").to_owned()),
            "deno_module",
            vec![],
            vec![],
            vec![Extension::builder().ops(vec![rust_impl::decl()]).build()],
            DenoModuleSharedState::default(),
            None,
        )
        .await
        .unwrap();

        let sync_ret_value = deno_module
            .execute_function(
                "syncUsingRegisteredFunction",
                vec![Arg::Serde(Value::String("param".into()))],
            )
            .await
            .unwrap();
        assert_eq!(sync_ret_value, Value::String("Register Op: param".into()));
    }
}
