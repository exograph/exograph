// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_ast::EmitOptions;
use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSourceCode;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::RequestedModuleType;
use deno_core::ResolutionKind;
use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::error::CoreError;
use deno_core::error::ModuleLoaderError;
use deno_core::resolve_import;
use include_dir::Dir;
use std::collections::HashMap;
use std::path::PathBuf;

/// Loads TypeScript and TypeScript-compatible files by either downloading them
/// or reading them off disk.
pub struct TypescriptLoader {
    pub embedded_dirs: HashMap<String, &'static Dir<'static>>,
}

impl ModuleLoader for TypescriptLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, deno_core::error::ModuleLoaderError> {
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> deno_core::ModuleLoadResponse {
        enum Code<'a> {
            Slice(&'a [u8]),
            Vec(Vec<u8>),
            String(String),
        }

        impl Code<'_> {
            pub fn to_string(&self) -> Result<String, AnyError> {
                match self {
                    Code::Slice(slice) => Ok(std::str::from_utf8(slice)?.to_string()),
                    Code::Vec(vec) => Code::Slice(vec).to_string(),
                    Code::String(s) => Ok(s.to_string()),
                }
            }
        }

        let module_specifier = module_specifier.clone();
        let embedded_dirs = self.embedded_dirs.clone();

        fn load(
            module_specifier: ModuleSpecifier,
            embedded_dirs: HashMap<String, &Dir>,
        ) -> Result<ModuleSource, ModuleLoaderError> {
            let (source, media_type): (Code, MediaType) = match module_specifier.scheme() {
                "http" | "https" => {
                    let path = PathBuf::from(module_specifier.path());

                    let module_specifier_string = module_specifier.to_string();

                    let code = std::thread::spawn(move || {
                        let res =
                            reqwest::blocking::get(&module_specifier_string).map_err(|e| {
                                ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    anyhow!("Failed to fetch {}: {:?}", module_specifier_string, e),
                                )))
                            })?;

                        if !res.status().is_success() {
                            Err(ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                anyhow!(
                                    "Failed to fetch {}: {:?}",
                                    module_specifier_string,
                                    res.status()
                                ),
                            ))))
                        } else {
                            Ok(res
                                .bytes()
                                .map_err(|e| {
                                    ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                                        std::io::ErrorKind::Other,
                                        anyhow!(
                                            "Failed to fetch {}: {:?}",
                                            module_specifier_string,
                                            e
                                        ),
                                    )))
                                })?
                                .to_vec())
                        }
                    })
                    .join()
                    .map_err(|e| {
                        ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            anyhow!("Failed to fetch (thread join): {:?}", e),
                        )))
                    })?
                    .map_err(|e| {
                        ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            anyhow!("Failed to fetch  {:?}", e),
                        )))
                    })?;

                    (Code::Vec(code), MediaType::from_path(&path))
                }

                "file" => {
                    let path = module_specifier.to_file_path().map_err(|()| {
                        ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            anyhow!("Failed to get file path"),
                        )))
                    })?;

                    let code = std::fs::read(&path).ok().map(Code::Vec).ok_or_else(|| {
                        ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            anyhow!(
                                "Could not get contents of {} from filesystem",
                                path.display()
                            ),
                        )))
                    })?;

                    (code, MediaType::from_path(&path))
                }
                "embedded" => {
                    let host = module_specifier
                        .host()
                        .ok_or_else(|| {
                            ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                anyhow!("No key specified in embedded URL"),
                            )))
                        })?
                        .to_string();
                    let path = PathBuf::from(module_specifier.path()[1..].to_string()); // [1..]: trim the root slash

                    let code = embedded_dirs
                        .get(&host)
                        .and_then(|embedded_dir| {
                            embedded_dir
                                .get_file(&path)
                                .map(|source| Code::Slice(source.contents()))
                        })
                        .ok_or_else(|| {
                            ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                anyhow!("Could not get embedded contents of {}", path.display()),
                            )))
                        })?;

                    (code, MediaType::from_path(&path))
                }
                scheme => {
                    return Err(ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        anyhow!("Unknown protocol scheme {}", scheme),
                    ))));
                }
            };

            let (module_type, should_transpile) = match &media_type {
                MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
                    (ModuleType::JavaScript, false)
                }
                MediaType::Jsx => (ModuleType::JavaScript, true),
                MediaType::TypeScript
                | MediaType::Mts
                | MediaType::Cts
                | MediaType::Dts
                | MediaType::Dmts
                | MediaType::Dcts
                | MediaType::Tsx => (ModuleType::JavaScript, true),
                MediaType::Json => (ModuleType::Json, false),
                _ => {
                    return Err(ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        anyhow!("Unknown extension {:?}", media_type.as_ts_extension()),
                    ))));
                }
            };

            let source = if should_transpile {
                let source_code = match source {
                    Code::Slice(slice) => std::str::from_utf8(slice).map(|value| value.to_string()),
                    Code::Vec(vec) => std::str::from_utf8(&vec).map(|value| value.to_string()),
                    Code::String(s) => Ok(s),
                }
                .map_err(|e| {
                    ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        anyhow!("Failed to convert source to string: {:?}", e),
                    )))
                })?;

                let parsed = deno_ast::parse_module(ParseParams {
                    specifier: module_specifier.clone(),
                    text: source_code.into(),
                    media_type,
                    capture_tokens: false,
                    scope_analysis: false,
                    maybe_syntax: None,
                })
                .map_err(|e| {
                    ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        anyhow!("Failed to parse module: {:?}", e),
                    )))
                })?;

                Code::String(
                    parsed
                        .transpile(
                            &Default::default(),
                            &Default::default(),
                            &EmitOptions::default(),
                        )
                        .map_err(|e| {
                            ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                anyhow!("Failed to transpile module: {:?}", e),
                            )))
                        })?
                        .into_source()
                        .text,
                )
            } else {
                source
            };

            let source = source.to_string().map_err(|e| {
                ModuleLoaderError::Core(CoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    anyhow!("Failed to convert source to string: {:?}", e),
                )))
            })?;

            let module = ModuleSource::new(
                module_type,
                ModuleSourceCode::String(source.into()),
                &module_specifier,
                None,
            );

            Ok(module)
        }

        deno_core::ModuleLoadResponse::Sync(
            load(module_specifier, embedded_dirs)
                .map_err(|e| deno_core::error::ModuleLoaderError::Core(CoreError::from(e))),
        )
    }
}
