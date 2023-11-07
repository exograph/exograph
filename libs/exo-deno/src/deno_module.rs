// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::serde_json;
use deno_core::serde_v8;
use deno_core::url::Url;
use deno_core::v8;
use deno_core::Extension;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_fs::FileSystem;
use deno_npm::resolution::NpmResolutionSnapshot;
use deno_npm::resolution::PackageReqNotFoundError;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageCacheFolderId;
use deno_npm::NpmPackageId;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_io::Stdio;
use deno_runtime::deno_node::NpmResolver;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::PermissionsContainer;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReq;
use deno_semver::Version;
use deno_virtual_fs::file_system::DenoCompileFileSystem;
use deno_virtual_fs::virtual_fs::FileBackedVfs;
use deno_virtual_fs::virtual_fs::VfsRoot;
use include_dir::Dir;
use tempfile::tempfile;
use tracing::error;

use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use tracing::instrument;

use serde_json::Value;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs;
use std::rc::Rc;
use std::sync::Arc;

use crate::deno_error::DenoDiagnosticError;
use crate::deno_error::DenoError;
use crate::deno_error::DenoInternalError;
use crate::deno_executor_pool::DenoScriptDefn;
use crate::deno_executor_pool::ResolvedModule;

use super::embedded_module_loader::EmbeddedModuleLoader;

fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
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

#[derive(Debug)]
struct SnapshotNpmResolver {
    registry_base: PathBuf,
    snapshot: NpmResolutionSnapshot,
}

impl SnapshotNpmResolver {
    pub fn new(registry_base: PathBuf, serialized: SerializedNpmResolutionSnapshot) -> Self {
        let snapshot = NpmResolutionSnapshot::new(serialized.into_valid().unwrap());
        Self {
            registry_base,
            snapshot,
        }
    }
}

impl NpmResolver for SnapshotNpmResolver {
    fn resolve_package_folder_from_package(
        &self,
        specifier: &str,
        referrer: &ModuleSpecifier,
        mode: deno_runtime::deno_node::NodeResolutionMode,
    ) -> Result<PathBuf, AnyError> {
        assert!(mode == deno_runtime::deno_node::NodeResolutionMode::Execution);
        if let Ok(referrer_path) = referrer.to_file_path() {
            if let Ok(without_registry) = referrer_path.strip_prefix(&self.registry_base) {
                let first_two = without_registry.iter().take(2).collect::<Vec<_>>();
                let version_maybe_index = first_two[1].to_str().unwrap();
                let split = version_maybe_index.split('_').collect::<Vec<_>>();

                let referrer_id = NpmPackageCacheFolderId {
                    nv: PackageNv {
                        name: first_two[0].to_str().unwrap().to_string(),
                        version: Version::parse_standard(split[0]).unwrap(),
                    },
                    copy_index: if split.len() > 1 {
                        split[1].parse::<u8>().unwrap()
                    } else {
                        0
                    },
                };
                let resolved = self
                    .snapshot
                    .resolve_package_from_package(specifier, &referrer_id)?;

                Ok(self
                    .registry_base
                    .join(&resolved.id.nv.name)
                    .join(resolved.id.nv.version.to_string()))
            } else {
                bail!("Expected referrer module to also be in the registry")
            }
        } else {
            bail!("Expected referrer module to be a file path")
        }
    }

    fn resolve_package_folder_from_path(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<PathBuf>, AnyError> {
        let without_registry = path.strip_prefix(&self.registry_base).unwrap();
        let first_two = without_registry.iter().take(2).collect::<Vec<_>>();
        let full_path = self.registry_base.join(first_two[0]).join(first_two[1]);
        Ok(Some(full_path))
    }

    fn resolve_package_folder_from_deno_module(
        &self,
        _pkg_nv: &PackageNv,
    ) -> Result<PathBuf, AnyError> {
        panic!()
    }

    fn resolve_pkg_id_from_pkg_req(
        &self,
        _req: &PackageReq,
    ) -> Result<NpmPackageId, PackageReqNotFoundError> {
        panic!()
    }

    fn in_npm_package(&self, specifier: &ModuleSpecifier) -> bool {
        specifier
            .to_file_path()
            .is_ok_and(|p| p.starts_with(&self.registry_base))
    }

    fn ensure_read_permission(
        &self,
        _permissions: &dyn deno_runtime::deno_node::NodePermissions,
        path: &std::path::Path,
    ) -> Result<(), AnyError> {
        if path.starts_with(&self.registry_base) {
            Ok(())
        } else {
            bail!("")
        }
    }
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
///             Each source code must define an object with properties that become the property of the name of the shim.
/// * `additional_code` - Any additional code (such as definition of a global type or a global function) to load.
/// * `extensions` - A list of extensions to load.
/// * `shared_state` - A shared state object to pass to the worker.
/// * `explicit_error_class_name` - The name of the class whose message will be used to report errors.
/// * `embedded_script_dirs` - A HashMap containing include_dir!() directories to provide to the script.
///                            They may be accessed through the `embedded://` module specifier:
///                            ```ts
///                            import { example } from "embedded://key/path/in/directory";
///                            ```
/// * `extra_sources` - A Vec of (URL, code) pairs to include in the source map.
///                     As the source map is the first thing queried during module resolution, this is useful for overriding
///                     scripts at certain paths with your own version.
///
impl DenoModule {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mut user_code: UserCode,
        user_agent_name: &str,
        shims: Vec<(&str, &[&str])>,
        additional_code: Vec<&'static str>,
        extensions: Vec<Extension>,
        shared_state: DenoModuleSharedState,
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
                Url::from_file_path(&abs).unwrap()
            }
            UserCode::LoadFromMemory { path, .. } => Url::parse(path).unwrap(),
        };

        let source_code = format!(
            "import * as mod from '{user_module_path}'; globalThis.mod = mod; {shim_source_code}"
        );

        let main_module_specifier = "file:///main.js".to_string();
        let main_specifier_parsed = ModuleSpecifier::parse(&main_module_specifier)?;

        let (mut script_modules, npm_snapshot) = match &mut user_code {
            UserCode::LoadFromFs(_) => (
                vec![(
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
                None,
            ),
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

                (out.into_iter().collect(), script.npm_snapshot.take())
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

        let create_web_worker_cb = Arc::new(|_| {
            todo!("Web workers are not supported");
        });

        let (fs, resolver): (Arc<dyn FileSystem>, Option<Arc<dyn NpmResolver>>) =
            if let Some((resolution, vfs, contents)) = npm_snapshot {
                let mut temp_file = tempfile().unwrap();
                for buffer in contents {
                    temp_file.write_all(&buffer).unwrap();
                }

                #[cfg(target_os = "windows")]
                let absolute_root = PathBuf::from("C:\\EXOGRAPH_NPM_MODULES_SNAPSHOT");

                #[cfg(not(target_os = "windows"))]
                let absolute_root = PathBuf::from("/EXOGRAPH_NPM_MODULES_SNAPSHOT");

                (
                    Arc::new(DenoCompileFileSystem::new(FileBackedVfs::new(
                        temp_file,
                        VfsRoot {
                            dir: vfs,
                            root_path: absolute_root.clone(),
                            start_file_offset: 0,
                        },
                    ))),
                    Some(Arc::new(SnapshotNpmResolver::new(
                        absolute_root,
                        resolution,
                    ))),
                )
            } else {
                (Arc::new(deno_fs::RealFs), None)
            };

        let options = WorkerOptions {
            bootstrap: BootstrapOptions {
                args: vec![],
                cpu_count: 1,
                log_level: Default::default(),
                enable_testing_features: false,
                location: None,
                no_color: false,
                runtime_version: "x".to_string(),
                ts_version: "x".to_string(),
                unstable: true,
                is_tty: false,
                user_agent: user_agent_name.to_string(),
                inspect: false,
                locale: "en".to_string(),
                has_node_modules_dir: false,
                maybe_binary_npm_command_name: None,
            },
            create_params: None,
            extensions,
            unsafely_ignore_certificate_errors: None,
            root_cert_store_provider: None,
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
            source_map_getter: None,
            format_js_error_fn: None,
            fs,
            stdio: Stdio::default(),
            npm_resolver: resolver,
            cache_storage_dir: None,
            should_wait_for_inspector_session: false,
            startup_snapshot: None,
        };

        let main_module = deno_core::resolve_url(&main_module_specifier)?;
        let permissions = PermissionsContainer::allow_all();

        let mut worker =
            MainWorker::bootstrap_from_options(main_module.clone(), permissions, options);

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
            .execute_script("", format!("mod.{function_name}").into())
            .map_err(DenoInternalError::Any)?;

        let shim_objects: HashMap<_, _> = {
            let shim_objects_vals: Vec<_> = self
                .shim_object_names
                .iter()
                .map(|name| runtime.execute_script("", name.clone().into()))
                .collect::<Result<_, _>>()
                .map_err(DenoInternalError::Any)?;
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
            let value = runtime.resolve_value(global).await.map_err(|err| {
                // got some AnyError from Deno internals...
                error!(%err);

                // If the function is async, we will get access to the error here. If it is an JsError, we process
                // it to define the error returned to the user (just like we do for the sync case above).
                match err.downcast::<JsError>() {
                    Ok(err) => Self::process_js_error(self.explicit_error_class_name, err),
                    Err(err) => DenoError::AnyError(err),
                }
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

/// Set of shared resources between DenoModules.
/// Cloning one DenoModuleSharedState and providing it to a set of DenoModules will
/// give them all access to the state through Arc<>s!
#[derive(Clone, Default)]
pub struct DenoModuleSharedState {
    pub blob_store: Arc<BlobStore>,
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
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("direct.js")
                    .to_owned(),
            ),
            "deno_module",
            vec![],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
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
            "deno_module",
            vec![],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
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
            "deno_module",
            vec![GET_JSON_SHIM],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
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
            "deno_module",
            vec![GET_JSON_SHIM],
            vec![],
            vec![],
            DenoModuleSharedState::default(),
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

    #[op]
    fn rust_impl(arg: String) -> Result<String, AnyError> {
        Ok(format!("Register Op: {arg}"))
    }

    #[op]
    async fn async_rust_impl(arg: String) -> Result<String, AnyError> {
        Ok(format!("Register Async Op: {arg}"))
    }

    #[tokio::test]
    async fn test_register_sync_ops() {
        deno_core::extension!(test, ops = [rust_impl],);
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("through_rust_fn.js")
                    .to_owned(),
            ),
            "deno_module",
            vec![],
            vec![],
            vec![test::init_ops()],
            DenoModuleSharedState::default(),
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
        deno_core::extension!(test, ops = [async_rust_impl],);
        let mut deno_module = DenoModule::new(
            UserCode::LoadFromFs(
                Path::new("src")
                    .join("test_js")
                    .join("through_rust_fn.js")
                    .to_owned(),
            ),
            "deno_module",
            vec![],
            vec![],
            vec![test::init_ops()],
            DenoModuleSharedState::default(),
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
