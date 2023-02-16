use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_import;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;

use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use include_dir::Dir;

/// A module loader that allows loading source code from memory for the given module specifier;
/// otherwise, loading it from an FsModuleLoader
/// Based on https://deno.land/x/deno@v1.15.0/cli/standalone.rs
pub(super) struct EmbeddedModuleLoader {
    pub embedded_dirs: HashMap<String, &'static Dir<'static>>,
    pub source_code_map: Arc<RefCell<HashMap<ModuleSpecifier, Vec<u8>>>>,
}

impl ModuleLoader for EmbeddedModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, AnyError> {
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<ModuleSpecifier>,
        is_dynamic: bool,
    ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
        // do we have the module source in-memory?
        if let Some(script) = self.source_code_map.borrow().get(module_specifier) {
            // return copy of module source in memory

            let script = script.clone();
            let module_specifier = module_specifier.clone();
            async move {
                Ok(ModuleSource {
                    code: script.into(),
                    module_url_specified: module_specifier.to_string(),
                    module_url_found: module_specifier.to_string(),
                    module_type: ModuleType::JavaScript,
                })
            }
            .boxed_local()
        } else {
            // we will have to load it ourselves

            let source_code_map = self.source_code_map.clone();
            let module_specifier = module_specifier.clone();
            let embedded_dirs = self.embedded_dirs.clone();

            async move {
                #[cfg(feature = "typescript-loader")]
                let loader = crate::typescript_module_loader::TypescriptLoader { embedded_dirs };

                #[cfg(not(feature = "typescript-loader"))]
                let loader = deno_core::FsModuleLoader;

                // use the configured loader to load the script from an external source
                let module_source = loader
                    .load(&module_specifier, maybe_referrer, is_dynamic)
                    .await?;

                // cache result for later
                let mut map = source_code_map.borrow_mut();
                map.insert(module_specifier, module_source.code.clone().into());

                Ok(module_source)
            }
            .boxed_local()
        }
    }
}
