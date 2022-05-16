use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::FsModuleLoader;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;

use std::collections::HashMap;
use std::pin::Pin;

/// A module loader that allows loading source code from memory for the given module specifier;
/// otherwise, loading it from an FsModuleLoader
/// Based on https://deno.land/x/deno@v1.15.0/cli/standalone.rs
pub(super) struct EmbeddedModuleLoader {
    pub source_code_map: HashMap<String, String>,
}

impl ModuleLoader for EmbeddedModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        is_main: bool,
    ) -> Result<ModuleSpecifier, AnyError> {
        // If the specifier matches this modules specifier, return that
        if let Ok(module_specifier) = deno_core::resolve_url(specifier) {
            if self.source_code_map.get(specifier).is_some() {
                return Ok(module_specifier);
            }
        }
        FsModuleLoader.resolve(specifier, referrer, is_main)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<ModuleSpecifier>,
        is_dynamic: bool,
    ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
        // If the specifier matches this modules specifier, return the source code
        if let Some(script) = self.source_code_map.get(module_specifier.as_str()) {
            let module_specifier = module_specifier.clone();
            let script = script.to_string();
            async move {
                let specifier = module_specifier.to_string();

                Ok(ModuleSource {
                    code: script.into_bytes().into_boxed_slice(),
                    module_url_specified: specifier.clone(),
                    module_url_found: specifier,
                    module_type: ModuleType::JavaScript,
                })
            }
            .boxed_local()
        } else {
            FsModuleLoader.load(module_specifier, maybe_referrer, is_dynamic)
        }
    }
}
