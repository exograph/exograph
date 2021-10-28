use deno_core::error::AnyError;
use deno_core::error::JsError;
use deno_core::serde_json;
use deno_core::FsModuleLoader;
use deno_core::JsRuntime;
use deno_runtime::ops::worker_host::CreateWebWorkerArgs;
use deno_runtime::web_worker::SendableWebWorkerHandle;
use deno_runtime::web_worker::WebWorker;
use deno_runtime::web_worker::WebWorkerOptions;
use deno_runtime::web_worker::WebWorkerType;
use std::sync::Mutex;

use deno_core::v8::Global;
use deno_core::v8::Script;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use serde_json::Value;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use std::convert::TryFrom;

use anyhow::{anyhow, Result};

use deno_core::v8;

use crate::embedded_module_loader::EmbeddedModuleLoader;

fn get_error_class_name(e: &AnyError) -> &'static str {
    deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

// From https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Number
const JS_MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;
const JS_MIN_SAFE_INTEGER: i64 = -JS_MAX_SAFE_INTEGER;
const JS_MAX_VALUE: f64 = 1.797_693_134_862_315_7e308;
const JS_MIN_VALUE: f64 = 5e-324;

pub struct DenoModule {
    worker: Arc<Mutex<MainWorker>>,
    shim_object_names: Vec<String>,
    script_map: HashMap<String, DenoScript>,
}

pub struct DenoScript {
    pub script: Global<Script>,
}

lazy_static::lazy_static! {
    static ref BOOTSTRAP: BootstrapOptions = BootstrapOptions {
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
    };
}

fn create_web_worker(args: CreateWebWorkerArgs) -> (WebWorker, SendableWebWorkerHandle) {
    let module_loader = Rc::new(FsModuleLoader);

    WebWorker::bootstrap_from_options(
        args.name,
        Permissions::allow_all(),
        args.main_module,
        args.worker_id,
        WebWorkerOptions {
            bootstrap: BOOTSTRAP.clone(),
            extensions: vec![],
            unsafely_ignore_certificate_errors: None,
            root_cert_store: None,
            user_agent: "Claytip".into(),
            seed: None,
            module_loader,
            create_web_worker_cb: Arc::new(Box::new(create_web_worker)),
            js_error_create_fn: None,
            use_deno_namespace: false,
            worker_type: WebWorkerType::Module,
            maybe_inspector_server: None,
            get_error_class_fn: None,
            blob_store: BlobStore::default(),
            broadcast_channel: InMemoryBroadcastChannel::default(),
            shared_array_buffer_store: None,
            compiled_wasm_module_store: None,
        },
    )
}

impl DenoModule {
    pub async fn new(
        user_module_path: &Path,
        user_agent_name: &str,
        shims: &[(&str, &str)],
        register_ops: &dyn Fn(&mut JsRuntime),
    ) -> Result<Self, AnyError> {
        let user_module_path = fs::canonicalize(user_module_path)?
            .to_string_lossy()
            .to_string();

        let shim_source_code = {
            let shims_source_codes: Vec<_> = shims
                .iter()
                .map(|(shim_name, source)| format!("globalThis.{} = {};", shim_name, source))
                .collect();

            shims_source_codes.join("\n")
        };

        let source_code = format!(
            "import * as mod from '{}'; globalThis.mod = mod; {}",
            user_module_path, shim_source_code
        );

        let main_module_specifier = "file:///main.js".to_string();
        let module_loader = Rc::new(EmbeddedModuleLoader {
            source_code,
            module_specifier: main_module_specifier.clone(),
        });

        let create_web_worker_cb = Arc::new(Box::new(|args: CreateWebWorkerArgs| {
            create_web_worker(args)
        }));

        let options = WorkerOptions {
            bootstrap: BOOTSTRAP.clone(),
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
            blob_store: BlobStore::default(),
            broadcast_channel: InMemoryBroadcastChannel::default(),
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
        worker.run_event_loop(false).await?;

        let shim_object_names = shims.iter().map(|(name, _)| name.to_string()).collect();

        Ok(Self {
            worker: Arc::new(Mutex::new(worker)),
            shim_object_names,
            script_map: HashMap::new(),
        })
    }

    pub fn preload_function(&mut self, function_names: Vec<&str>) {
        let worker = &mut self.worker;
        let runtime = &mut worker.lock().unwrap().js_runtime;

        for fname in function_names.iter() {
            let script = preload_script(runtime, fname, &format!("mod.{}", fname));
            self.script_map.insert(fname.to_string(), script);
        }
    }

    pub async fn execute_function(&mut self, function_name: &str, args: Vec<Arg>) -> Result<Value> {
        let worker = &mut self.worker;
        let runtime = &mut worker.lock().unwrap().js_runtime;

        // TODO: does this yield any significant optimization?
        let func_value = run_script(runtime, &self.script_map[function_name]).unwrap();

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
                        .get(tc_scope_ref)
                        .to_object(tc_scope_ref)
                        .unwrap()
                        .into()),
                })
                .collect::<Result<Vec<_>, AnyError>>()?;

            let func_obj = func_value
                .get(tc_scope_ref)
                .to_object(tc_scope_ref)
                .unwrap();
            let func = v8::Local::<v8::Function>::try_from(func_obj)?;

            let undefined = v8::undefined(tc_scope_ref);
            let local = func.call(tc_scope_ref, undefined.into(), &args);

            let local = match local {
                Some(value) => value,
                None => {
                    let exception = tc_scope_ref.exception().unwrap();
                    let js_error = JsError::from_v8_exception(tc_scope_ref, exception);
                    return Err(anyhow!(js_error));
                }
            };

            v8::Global::new(tc_scope_ref, local)
        };

        {
            let value = runtime.resolve_value(global).await?;
            let scope = &mut runtime.handle_scope();

            let res = v8::Local::new(scope, value);

            let res: Value = serde_v8::from_v8(scope, res)?;
            Ok(res)
        }
    }
}

fn preload_script(runtime: &mut JsRuntime, name: &str, source_code: &str) -> DenoScript {
    let mut scope = runtime.handle_scope();

    let source = v8::String::new(&mut scope, source_code).unwrap();
    let name = v8::String::new(&mut scope, name).unwrap();

    let source_map_url = v8::String::new(&mut scope, "").unwrap();
    let origin = v8::ScriptOrigin::new(
        &mut scope,
        name.into(),
        0,
        0,
        false,
        123,
        source_map_url.into(),
        true,
        false,
        false,
    );

    let mut tc_scope = v8::TryCatch::new(&mut scope);

    match v8::Script::compile(&mut tc_scope, source, Some(&origin)) {
        Some(local_script) => {
            let script = v8::Global::new(&mut tc_scope, local_script);
            DenoScript { script }
        }
        None => panic!(),
    }
}

fn run_script(runtime: &mut JsRuntime, ds: &DenoScript) -> Result<Global<v8::Value>> {
    let mut scope = runtime.handle_scope();
    let mut tc_scope = v8::TryCatch::new(&mut scope);

    match ds.script.get(&mut tc_scope).run(&mut tc_scope) {
        Some(value) => {
            let value_handle = v8::Global::new(&mut tc_scope, value);
            Ok(value_handle)
        }
        None => {
            panic!("Exception");
        }
    }
}

pub enum Arg {
    Serde(serde_json::Value),
    Shim(String),
}
