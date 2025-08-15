// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_import;
use deno_core::url::Url;
use deno_runtime::deno_node::NodeResolver;
use node_resolver::NodeModuleKind;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use include_dir::Dir;

use crate::deno_executor_pool::ResolvedModule;

/// A module loader that allows loading source code from memory for the given module specifier;
/// otherwise, loading it from an FsModuleLoader
/// Based on <https://deno.land/x/deno@v1.15.0/cli/standalone.rs>
pub(super) struct EmbeddedModuleLoader {
    #[allow(unused)]
    pub embedded_dirs: HashMap<String, &'static Dir<'static>>,
    pub source_code_map: Rc<RefCell<HashMap<Url, ResolvedModule>>>,
    pub node_resolver: Option<Arc<NodeResolver>>,
}

impl ModuleLoader for EmbeddedModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, AnyError> {
        if let Some(node_resolver) = &self.node_resolver
            && let Ok(referrer) = ModuleSpecifier::parse(referrer)
            && node_resolver.in_npm_package(&referrer)
            && let Ok(res) = node_resolver.resolve(
                specifier,
                &referrer,
                NodeModuleKind::Esm,
                node_resolver::NodeResolutionMode::Execution,
            )
        {
            return Ok(res.into_url());
        }

        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        maybe_referrer: Option<&ModuleSpecifier>,
        is_dynamic: bool,
        _requested_module_type: RequestedModuleType,
    ) -> deno_core::ModuleLoadResponse {
        let borrowed_map = self.source_code_map.borrow();

        #[allow(unused_mut)]
        let mut module_specifier_unix = module_specifier.clone();
        #[cfg(target_os = "windows")]
        {
            module_specifier_unix =
                ModuleSpecifier::parse(&module_specifier_unix.as_str().replace(
                    "/C:/EXOGRAPH_NPM_MODULES_SNAPSHOT",
                    "/EXOGRAPH_NPM_MODULES_SNAPSHOT",
                ))
                .unwrap();
        }

        let mut resolved = borrowed_map.get(&module_specifier_unix);
        while let Some(ResolvedModule::Redirect(to)) = resolved {
            resolved = borrowed_map.get(to);
        }

        // do we have the module source in-memory?
        let module_specifier = module_specifier.clone();

        if let Some(script) = resolved {
            #[allow(unused, unused_mut)]
            if let ResolvedModule::Module(
                mut script,
                module_type,
                mut final_specifier,
                requires_rewrite,
            ) = script.clone()
            {
                // on windows, we need to rewrite the absolute path to use C:\\ instead of /
                #[cfg(target_os = "windows")]
                if requires_rewrite {
                    script = script.replace(
                        "/EXOGRAPH_NPM_MODULES_SNAPSHOT",
                        "C:\\\\EXOGRAPH_NPM_MODULES_SNAPSHOT",
                    );
                }

                #[cfg(target_os = "windows")]
                {
                    final_specifier = ModuleSpecifier::parse(&final_specifier.as_str().replace(
                        "/EXOGRAPH_NPM_MODULES_SNAPSHOT",
                        "C:\\EXOGRAPH_NPM_MODULES_SNAPSHOT",
                    ))
                    .unwrap();
                }

                let module_source = ModuleSource::new_with_redirect(
                    module_type,
                    deno_core::ModuleSourceCode::String(script.into()),
                    &module_specifier,
                    &final_specifier,
                    None,
                );
                // TODO: Can we use Sync here?
                deno_core::ModuleLoadResponse::Async(async move { Ok(module_source) }.boxed_local())
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

            let module_load_future = async move {
                #[cfg(feature = "typescript-loader")]
                let loader = crate::typescript_module_loader::TypescriptLoader { embedded_dirs };

                #[cfg(not(feature = "typescript-loader"))]
                let loader = deno_core::FsModuleLoader;

                // use the configured loader to load the script from an external source
                let module_load_response = loader.load(
                    &module_specifier,
                    maybe_referrer.as_ref(),
                    is_dynamic,
                    _requested_module_type,
                );

                let module_source = match module_load_response {
                    deno_core::ModuleLoadResponse::Sync(module_source) => module_source?,
                    deno_core::ModuleLoadResponse::Async(module_load_future) => {
                        module_load_future.await?
                    }
                };

                // cache result for later
                let mut map = source_code_map.borrow_mut();

                let source_code = match module_source.code {
                    deno_core::ModuleSourceCode::String(ref code) => code.as_str().to_string(),
                    deno_core::ModuleSourceCode::Bytes(ref bytes) => {
                        String::from_utf8(bytes.to_vec())?
                    }
                };

                map.insert(
                    module_specifier.clone(),
                    ResolvedModule::Module(
                        source_code.as_str().to_string(),
                        module_source.module_type.clone(),
                        module_specifier,
                        false,
                    ),
                );

                Ok(module_source)
            };
            deno_core::ModuleLoadResponse::Async(module_load_future.boxed_local())
        }
    }
}
