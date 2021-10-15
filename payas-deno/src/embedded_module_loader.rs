/// Embedded Module loader is a basically FsModuleLoader with support for loading source code from memory.
/// Based on https://deno.land/x/deno@v1.15.0/cli/standalone.rs
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::FsModuleLoader;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;

use std::pin::Pin;

pub struct EmbeddedModuleLoader {
    pub source_code: String,
    pub underlying_module_loader: FsModuleLoader,
    pub module_specifier: String,
}

impl ModuleLoader for EmbeddedModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        is_main: bool,
    ) -> Result<ModuleSpecifier, AnyError> {
        // If this is the source code module, resolve it here
        if let Ok(module_specifier) = deno_core::resolve_url(specifier) {
            if specifier == self.module_specifier {
                return Ok(module_specifier);
            }
        }
        self.underlying_module_loader
            .resolve(specifier, referrer, is_main)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<ModuleSpecifier>,
        is_dynamic: bool,
    ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
        let module_specifier = module_specifier.clone();

        // If this is not the source code module, delegate to underlying module loader
        if module_specifier.to_string() != self.module_specifier {
            return self.underlying_module_loader.load(
                &module_specifier,
                maybe_referrer,
                is_dynamic,
            );
        }
        let code = self.source_code.to_string();
        async move {
            let specifier = module_specifier.to_string();

            Ok(ModuleSource {
                code,
                module_url_specified: specifier.clone(),
                module_url_found: specifier,
            })
        }
        .boxed_local()
    }
}
