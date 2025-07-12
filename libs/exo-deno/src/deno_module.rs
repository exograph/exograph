// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_core::Extension;
use deno_core::ModuleLoader;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::serde_json;
use deno_core::serde_v8;
use deno_core::url::Url;
use deno_core::v8;
use deno_resolver::npm::DenoInNpmPackageChecker;
use deno_resolver::npm::NpmResolver;
use deno_runtime::deno_permissions::PermissionsContainer;
use deno_runtime::permissions::RuntimePermissionDescriptorParser;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::worker::WorkerServiceOptions;
use include_dir::Dir;
use tracing::error;

use std::cell::RefCell;
use std::path::PathBuf;
use tracing::instrument;

use serde_json::Value;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::rc::Rc;
use std::sync::Arc;

use crate::deno_executor_pool::DenoScriptDefn;
use crate::deno_executor_pool::ResolvedModule;
use crate::error::DenoDiagnosticError;
use crate::error::DenoError;
use crate::error::DenoInternalError;

use super::embedded_module_loader::EmbeddedModuleLoader;
use deno_error::JsErrorBox;
use node_resolver::errors::ClosestPkgJsonError;

/// Minimal implementation of NodeRequireLoader for compatibility
struct BasicNodeRequireLoader;

impl deno_runtime::deno_node::NodeRequireLoader for BasicNodeRequireLoader {
    fn ensure_read_permission<'a>(
        &self,
        _permissions: &mut dyn deno_runtime::deno_node::NodePermissions,
        path: &'a std::path::Path,
    ) -> Result<std::borrow::Cow<'a, std::path::Path>, JsErrorBox> {
        // Allow all file access for simplicity
        Ok(std::borrow::Cow::Borrowed(path))
    }

    fn load_text_file_lossy(
        &self,
        path: &std::path::Path,
    ) -> Result<deno_core::FastString, JsErrorBox> {
        // Read file content
        let content = std::fs::read_to_string(path)
            .map_err(|e| JsErrorBox::generic(format!("Failed to read file: {}", e)))?;
        Ok(deno_core::FastString::from(content))
    }

    fn is_maybe_cjs(&self, _specifier: &deno_core::url::Url) -> Result<bool, ClosestPkgJsonError> {
        // For simplicity, assume CommonJS modules are not used
        Ok(false)
    }
}

#[derive(Debug)]
pub enum UserCode {
    LoadFromMemory {
        script: DenoScriptDefn,
        path: String,
    },
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
/// DenoModule has no concept of Exograph; it exists solely to configure the JavaScript execution environment
/// and to load & execute methods in the Deno runtime from sources.
///
/// # Arguments
/// * `user_code` - The user code with exported functions (which may then be invoked using `DenoModule.execute_function` ).
/// * `user_agent_name` - The name of the user agent
/// * `shims` - A list of shims to load (each tuple is the name of the shim and a list of the source code).
///   Each source code must define an object with properties that become the property of the name of the shim.
/// * `additional_code` - Any additional code (such as definition of a global type or a global function) to load.
/// * `extensions` - A list of extensions to load.
/// * `shared_state` - A shared state object to pass to the worker.
/// * `explicit_error_class_name` - The name of the class whose message will be used to report errors.
/// * `embedded_script_dirs` - A HashMap containing include_dir!() directories to provide to the script.
///   They may be accessed through the `embedded://` module specifier:
///   ```ts
///   import { example } from "embedded://key/path/in/directory";
///   ```
/// * `extra_sources` - A Vec of (URL, code) pairs to include in the source map.
///   As the source map is the first thing queried during module resolution, this is useful for overriding
///   scripts at certain paths with your own version.
impl DenoModule {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mut user_code: UserCode,
        shims: Vec<(&str, &[&str])>,
        additional_code: Vec<&'static str>,
        extensions: Vec<Extension>,
        explicit_error_class_name: Option<&'static str>,
        embedded_script_dirs: Option<HashMap<String, &'static Dir<'static>>>,
        extra_sources: Option<Vec<(&str, String)>>,
    ) -> Result<Self, AnyError> {
        let shim_source_code = {
            let shims_source_codes: Vec<_> = shims
                .iter()
                .flat_map(|(shim_name, sources)| {
                    sources.iter().enumerate().map(move |(index, source)| {
                        if index == 0 {
                            format!("globalThis.{shim_name} = {source};")
                        } else {
                            format!("Object.assign(globalThis.{shim_name}, {source});")
                        }
                    })
                })
                .collect();

            shims_source_codes.join("\n")
        };

        let user_module_path = match &user_code {
            UserCode::LoadFromFs(user_module_path) => {
                let abs = fs::canonicalize(user_module_path)?;
                Url::from_file_path(abs).unwrap()
            }
            UserCode::LoadFromMemory { path, .. } => Url::parse(path).unwrap(),
        };

        let source_code = format!(
            "import * as mod from '{user_module_path}'; globalThis.mod = mod; {shim_source_code}"
        );

        let main_module_specifier = "file:///main.js".to_string();
        let main_specifier_parsed = ModuleSpecifier::parse(&main_module_specifier)?;

        let mut script_modules = match &mut user_code {
            UserCode::LoadFromFs(_) => vec![(
                main_specifier_parsed.clone(),
                ResolvedModule::Module(
                    source_code,
                    ModuleType::JavaScript,
                    main_specifier_parsed,
                    false,
                ),
            )]
            .into_iter()
            .collect::<HashMap<ModuleSpecifier, ResolvedModule>>(),
            UserCode::LoadFromMemory { script, .. } => {
                let mut out = vec![(
                    main_specifier_parsed.clone(),
                    ResolvedModule::Module(
                        source_code,
                        ModuleType::JavaScript,
                        main_specifier_parsed,
                        false,
                    ),
                )];

                for (specifier, resolved) in &script.modules {
                    out.push((specifier.clone(), resolved.clone()));
                }

                out.into_iter().collect()
            }
        };

        // override entries with provided extra_sources
        if let Some(extra_sources) = extra_sources {
            for (url, source) in extra_sources {
                script_modules.insert(
                    ModuleSpecifier::parse(url)?,
                    ResolvedModule::Module(
                        source,
                        ModuleType::JavaScript,
                        ModuleSpecifier::parse(url)?,
                        false,
                    ),
                );
            }
        }

        let module_loader = Rc::new(EmbeddedModuleLoader {
            source_code_map: Rc::new(RefCell::new(script_modules)),
            embedded_dirs: embedded_script_dirs.unwrap_or_default(),
        });

        let worker_options = WorkerOptions {
            startup_snapshot: Some(crate::deno_snapshot()),
            extensions,
            ..Default::default()
        };

        let main_module = deno_core::resolve_url(&main_module_specifier)?;

        let services = Self::worker_service_options(module_loader);

        let mut worker = MainWorker::bootstrap_from_options(&main_module, services, worker_options);

        // Ensure sys_traits::impls::RealSys is available in the op_state before any operations
        {
            let runtime = &mut worker.js_runtime;
            let op_state_ref = runtime.op_state();
            let mut op_state = op_state_ref
                .try_borrow_mut()
                .map_err(DenoDiagnosticError::BorrowMutError)?;

            op_state.put(sys_traits::impls::RealSys);

            // Add a basic NodeRequireLoader implementation to satisfy deno_node requirements
            let node_require_loader: Rc<dyn deno_runtime::deno_node::NodeRequireLoader> =
                Rc::new(BasicNodeRequireLoader);
            op_state.put(node_require_loader);
        }

        worker.execute_main_module(&main_module).await?;

        additional_code.iter().for_each(|code| {
            worker
                .execute_script("", deno_core::FastString::from_static(code))
                .unwrap();
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

    fn worker_service_options(
        module_loader: Rc<dyn ModuleLoader>,
    ) -> WorkerServiceOptions<
        DenoInNpmPackageChecker,
        NpmResolver<sys_traits::impls::RealSys>,
        sys_traits::impls::RealSys,
    > {
        let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
            sys_traits::impls::RealSys,
        ));
        let fs = Arc::new(deno_fs::RealFs);

        WorkerServiceOptions {
            deno_rt_native_addon_loader: None,
            module_loader,
            permissions: PermissionsContainer::allow_all(permission_desc_parser),
            blob_store: Default::default(),
            broadcast_channel: Default::default(),
            feature_checker: Default::default(),
            node_services: Default::default(),
            npm_process_state_provider: Default::default(),
            root_cert_store_provider: Default::default(),
            fetch_dns_resolver: Default::default(),
            shared_array_buffer_store: Default::default(),
            compiled_wasm_module_store: Default::default(),
            v8_code_cache: Default::default(),
            fs,
        }
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
    pub async fn execute_function(
        &mut self,
        function_name: &str,
        args: Vec<Arg>,
    ) -> Result<Value, DenoError> {
        let worker = &mut self.worker;
        let runtime = &mut worker.js_runtime;

        let func_value = runtime
            .execute_script(
                "",
                deno_core::FastString::from(format!("mod.{function_name}")),
            )
            .map_err(DenoInternalError::CoreError)?;

        let shim_objects: HashMap<_, _> = {
            let shim_objects_vals: Vec<_> = self
                .shim_object_names
                .iter()
                .map(|name| runtime.execute_script("", deno_core::FastString::from(name.clone())))
                .collect::<Result<_, _>>()
                .map_err(DenoInternalError::CoreError)?;
            self.shim_object_names
                .iter()
                .zip(shim_objects_vals.into_iter())
                .collect()
        };

        let global = {
            let scope = &mut runtime.handle_scope();

            let mut tc_scope = v8::TryCatch::new(scope);
            let tc_scope_ref = &mut tc_scope;

            let args: Vec<_> = args
                .into_iter()
                .map(|v| match v {
                    Arg::Serde(v) => {
                        Ok(serde_v8::to_v8(tc_scope_ref, v).map_err(DenoInternalError::Serde)?)
                    }
                    Arg::Shim(name) => Ok(shim_objects
                        .get(&name)
                        .ok_or(DenoDiagnosticError::MissingShim(name))?
                        .open(tc_scope_ref)
                        .to_object(tc_scope_ref)
                        .unwrap()
                        .into()),
                })
                .collect::<Result<Vec<_>, DenoError>>()?;

            let func_obj = func_value
                .open(tc_scope_ref)
                .to_object(tc_scope_ref)
                .ok_or_else(|| {
                    DenoDiagnosticError::MissingFunction(
                        function_name.to_owned(),
                        match &self.user_code {
                            UserCode::LoadFromMemory { path, .. } => path,
                            UserCode::LoadFromFs(path) => path.to_str().unwrap(),
                        }
                        .to_owned(),
                    )
                })?;
            let func = v8::Local::<v8::Function>::try_from(func_obj)
                .map_err(DenoInternalError::DataError)?;

            let undefined = v8::undefined(tc_scope_ref);
            let local = func.call(tc_scope_ref, undefined.into(), &args);

            let local = match local {
                Some(value) => value,
                None => {
                    // We will get the exception here for sync functions
                    let exception = tc_scope_ref.exception().unwrap();
                    let js_error = JsError::from_v8_exception(tc_scope_ref, exception);

                    error!(%js_error, "Exception executing function");

                    return Err(Self::process_js_error(
                        self.explicit_error_class_name,
                        js_error,
                    ));
                }
            };

            v8::Global::new(tc_scope_ref, local)
        };

        {
            #[allow(deprecated)]
            // Deno's code also uses the deprecated function in their tests. We will reconsider this when their code remove this function.
            // See: https://github.com/denoland/deno_core/blob/main/core/benches/ops/async.rs
            let value = runtime
                .resolve_value(global)
                .await
                .map_err(|err| match err {
                    deno_core::error::CoreError::Js(js_error) => {
                        error!(%js_error, "Exception executing function");

                        Self::process_js_error(self.explicit_error_class_name, js_error)
                    }
                    _ => DenoError::AnyError(err.into()),
                })?;

            let scope = &mut runtime.handle_scope();
            let res = v8::Local::new(scope, value);
            let res: Value = serde_v8::from_v8(scope, res).map_err(DenoInternalError::Serde)?;
            Ok(res)
        }
    }

    /// Put a single instance of a type into Deno's op_state
    pub fn put<T: 'static>(&mut self, val: T) -> Result<(), DenoError> {
        self.worker
            .js_runtime
            .op_state()
            .try_borrow_mut()
            .map_err(DenoDiagnosticError::BorrowMutError)?
            .put(val);
        Ok(())
    }

    /// Try to take a single instance of a type from Deno's op_state
    pub fn take<T: 'static>(&mut self) -> Result<Option<T>, DenoError> {
        Ok(self
            .worker
            .js_runtime
            .op_state()
            .try_borrow_mut()
            .map_err(DenoDiagnosticError::BorrowMutError)?
            .try_take())
    }

    fn process_js_error(
        explicit_error_class_name: Option<&'static str>,
        js_error: JsError,
    ) -> DenoError {
        match explicit_error_class_name {
            Some(_) if js_error.name.as_deref() == explicit_error_class_name => {
                // code threw an explicit error, expose it to user
                let message = js_error
                    .message
                    .unwrap_or_else(|| "Unknown error".to_string());
                DenoError::Explicit(message)
            }
            _ => {
                // generic error message
                DenoError::JsError(js_error)
            }
        }
    }
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

    use deno_core::op2;
    use serde_json::json;
    use test_log::test;

    use super::*;

    #[test(tokio::test)]
    async fn test_direct_sync() {
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("direct.js")
                    .to_owned(),
            ),
            vec![],
            vec![],
            vec![],
            None,
            None,
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
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("direct.js")
                    .to_owned(),
            ),
            vec![],
            vec![],
            vec![],
            None,
            None,
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
        static GET_JSON_SHIM: (&str, &[&str]) = ("__shim", &[include_str!("./test_js/shim.js")]);

        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("through_shim.js")
                    .to_owned(),
            ),
            vec![GET_JSON_SHIM],
            vec![],
            vec![],
            None,
            None,
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
        static GET_JSON_SHIM: (&str, &[&str]) = ("__shim", &[include_str!("./test_js/shim.js")]);

        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("through_shim.js")
                    .to_owned(),
            ),
            vec![GET_JSON_SHIM],
            vec![],
            vec![],
            None,
            None,
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

    // #[op2]
    // #[string]
    // fn op_rust_impl(#[string] arg: String) -> Result<String, AnyError> {
    //     Ok(format!("Register Op: {arg}"))
    // }

    // #[op2(async)]
    // #[string]
    // async fn op_async_rust_impl(#[string] arg: String) -> Result<String, AnyError> {
    //     Ok(format!("Register Async Op: {arg}"))
    // }

    #[op2]
    #[string]
    fn op_rust_impl(#[string] arg: String) -> String {
        format!("Register Op: {arg}")
    }

    #[op2(async)]
    #[string]
    async fn op_async_rust_impl(#[string] arg: String) -> String {
        format!("Register Async Op: {arg}")
    }

    deno_core::extension!(
        test,
        ops = [op_rust_impl, op_async_rust_impl],
        esm_entry_point = "ext:test/_init.js",
        esm = [
            dir "src/test_js",
            "_init.js",
            "test:through_rust.js" = "through_rust.js",
        ]
    );

    #[tokio::test]
    async fn test_register_sync_ops() {
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("through_rust_fn.js")
                    .to_owned(),
            ),
            vec![],
            vec![],
            vec![test::init()],
            None,
            None,
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

    #[tokio::test]
    async fn test_register_async_ops() {
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("through_rust_fn.js")
                    .to_owned(),
            ),
            vec![],
            vec![],
            vec![test::init()],
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let async_ret_value = deno_module
            .execute_function(
                "asyncUsingRegisteredFunction",
                vec![Arg::Serde(Value::String("param".into()))],
            )
            .await
            .unwrap();
        assert_eq!(
            async_ret_value,
            Value::String("Register Async Op: param".into())
        );
    }
}
