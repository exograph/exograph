use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::JsRuntime;

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
    worker: MainWorker,
    shim_object_names: Vec<String>,
}

impl DenoModule {
    pub async fn new(
        user_module_path: &str,
        user_agent_name: &str,
        shims: &[(&str, &str)],
        register_ops: fn(&mut JsRuntime) -> (),
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
                unstable: false,
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
            worker,
            shim_object_names,
        })
    }

    pub async fn execute_function(
        &mut self,
        function_name: &str,
        args: Vec<Arg>,
    ) -> Result<Value, AnyError> {
        let worker = &mut self.worker;

        let runtime = &mut worker.js_runtime;

        let func_value = runtime
            .execute_script("", &format!("mod.{}", function_name))
            .unwrap();

        let shim_objects: HashMap<_, _> = self
            .shim_object_names
            .iter()
            .map(|name| (name, runtime.execute_script("", name).unwrap()))
            .collect();

        let global = {
            let scope = &mut runtime.handle_scope();
            let args: Vec<_> = args
                .into_iter()
                .map(|v| match v {
                    Arg::Serde(v) => match v {
                        // If we enable the arbitrary_precision feature for serde_json, then serde_v8::to_v8 will serialize numbers as
                        // { "$serde_json::private::Number": "<number>"}, which the JS side will not understand, so apply custom logic
                        // to get the underlying value and then use primitive serialization
                        // TODO: Check for
                        Value::Number(n) => if n.is_i64() {
                            let value = n.as_i64().unwrap();

                            if value >= JS_MIN_SAFE_INTEGER || value <= JS_MAX_SAFE_INTEGER {
                                serde_v8::to_v8(scope, value)
                            } else {
                                Err(serde_v8::Error::Message(format!(
                                    "Integer {} too large to safely convert to JavaScript",
                                    value
                                )))
                            }
                        } else if n.is_f64() {
                            let value = n.as_f64().unwrap();

                            if value >= JS_MIN_VALUE || value <= JS_MAX_VALUE {
                                serde_v8::to_v8(scope, value)
                            } else {
                                Err(serde_v8::Error::Message(format!(
                                    "Float {} too large to safely convert to JavaScript",
                                    value
                                )))
                            }
                        } else {
                            Err(serde_v8::Error::Message("Invalid number".into()))
                        }
                        .unwrap(),
                        _ => serde_v8::to_v8(scope, v).unwrap(),
                    },
                    Arg::Shim(name) => shim_objects
                        .get(&name)
                        .unwrap()
                        .get(scope)
                        .to_object(scope)
                        .unwrap()
                        .into(),
                })
                .collect();

            let func_obj = func_value.get(scope).to_object(scope).unwrap();
            let func = v8::Local::<v8::Function>::try_from(func_obj)?;

            let undefined = v8::undefined(scope);
            let local = func.call(scope, undefined.into(), &args).unwrap();

            v8::Global::new(scope, local)
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

pub enum Arg {
    Serde(serde_json::Value),
    Shim(String),
}
