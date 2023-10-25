// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_import;
use deno_core::url::Url;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ResolutionKind;

use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;

use include_dir::Dir;

use crate::deno_executor_pool::ResolvedModule;

/// A module loader that allows loading source code from memory for the given module specifier;
/// otherwise, loading it from an FsModuleLoader
/// Based on <https://deno.land/x/deno@v1.15.0/cli/standalone.rs>
pub(super) struct EmbeddedModuleLoader {
    #[allow(unused)]
    pub embedded_dirs: HashMap<String, &'static Dir<'static>>,
    pub source_code_map: Rc<RefCell<HashMap<Url, ResolvedModule>>>,
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
        maybe_referrer: Option<&ModuleSpecifier>,
        is_dynamic: bool,
    ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
        let borrowed_map = self.source_code_map.borrow();
        let mut resolved = borrowed_map.get(module_specifier);
        while let Some(ResolvedModule::Redirect(to)) = resolved {
            resolved = borrowed_map.get(to);
        }

        // do we have the module source in-memory?
        let module_specifier = module_specifier.clone();

        if let Some(script) = resolved {
            #[allow(unused_mut)]
            if let ResolvedModule::Module(mut script, module_type, mut final_specifier) =
                script.clone()
            {
                #[cfg(target_os = "windows")]
                {
                    script = script.replace(
                        "/EXOGRAPH_NPM_MODULES_SNAPSHOT",
                        "C:\\EXOGRAPH_NPM_MODULES_SNAPSHOT",
                    );
                    final_specifier = ModuleSpecifier::parse(final_specifier.as_str().replace(
                        "/EXOGRAPH_NPM_MODULES_SNAPSHOT",
                        "C:\\EXOGRAPH_NPM_MODULES_SNAPSHOT",
                    ))
                    .unwrap()
                }

                let module_source = ModuleSource::new_with_redirect(
                    module_type,
                    script.into(),
                    &module_specifier,
                    &final_specifier,
                );
                async move { Ok(module_source) }.boxed_local()
            } else {
                panic!()
            }
        } else {
            drop(borrowed_map);

            // we will have to load it ourselves
            let source_code_map = self.source_code_map.clone();
            let module_specifier = module_specifier.clone();

            #[cfg(feature = "typescript-loader")]
            let embedded_dirs = self.embedded_dirs.clone();

            let maybe_referrer = maybe_referrer.cloned();

            async move {
                #[cfg(feature = "typescript-loader")]
                let loader = crate::typescript_module_loader::TypescriptLoader { embedded_dirs };

                #[cfg(not(feature = "typescript-loader"))]
                let loader = deno_core::FsModuleLoader;

                // use the configured loader to load the script from an external source
                let module_source = loader
                    .load(&module_specifier, maybe_referrer.as_ref(), is_dynamic)
                    .await?;

                // cache result for later
                let mut map = source_code_map.borrow_mut();

                map.insert(
                    module_specifier.clone(),
                    ResolvedModule::Module(
                        module_source.code.as_str().to_string(),
                        module_source.module_type,
                        module_specifier,
                    ),
                );

                Ok(module_source)
            }
            .boxed_local()
        }
    }
}
