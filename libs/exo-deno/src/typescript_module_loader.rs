// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_ast::MediaType;
use deno_ast::ParseParams;
use deno_ast::SourceTextInfo;
use deno_core::anyhow::anyhow;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::resolve_import;
use deno_core::ModuleLoader;
use deno_core::ModuleSource;
use deno_core::ModuleSpecifier;
use deno_core::ModuleType;
use deno_core::ResolutionKind;
use futures::FutureExt;
use include_dir::Dir;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;

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
    ) -> Result<ModuleSpecifier, deno_core::anyhow::Error> {
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
    ) -> Pin<Box<deno_core::ModuleSourceFuture>> {
        enum Code<'a> {
            Slice(&'a [u8]),
            Vec(Vec<u8>),
            String(String),
        }

        impl<'a> Code<'a> {
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

        // adapted from https://github.com/denoland/deno/blob/v1.32.4/core/examples/ts_module_loader.rs
        fn load(
            module_specifier: ModuleSpecifier,
            embedded_dirs: HashMap<String, &Dir>,
        ) -> Result<ModuleSource, AnyError> {
            let (source, media_type): (Code, MediaType) = match module_specifier.scheme() {
                "http" | "https" => {
                    let path = PathBuf::from(module_specifier.path());

                    let mut writer = Vec::new();
                    let res = http_req::request::get(&module_specifier, &mut writer)?;

                    if !res.status_code().is_success() {
                        bail!("Failed to fetch {}: {}", module_specifier, res.reason())
                    }

                    (Code::Vec(writer), MediaType::from_path(&path))
                }

                "file" => {
                    let path = module_specifier
                        .to_file_path()
                        .map_err(|()| anyhow!("Failed to get file path"))?;

                    let code = std::fs::read(&path).ok().map(Code::Vec).ok_or_else(|| {
                        anyhow!(
                            "Could not get contents of {} from filesystem",
                            path.display()
                        )
                    })?;

                    (code, MediaType::from_path(&path))
                }

                "embedded" => {
                    let host = module_specifier
                        .host()
                        .ok_or_else(|| anyhow!("No key specified in embedded URL"))?
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
                            anyhow!("Could not get embedded contents of {}", path.display())
                        })?;

                    (code, MediaType::from_path(&path))
                }

                scheme => bail!("Unknown protocol scheme {}", scheme),
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
                _ => bail!("Unknown extension {:?}", media_type.as_ts_extension()),
            };

            let source = if should_transpile {
                let parsed = deno_ast::parse_module(ParseParams {
                    specifier: module_specifier.to_string(),
                    text_info: SourceTextInfo::from_string(source.to_string()?),
                    media_type,
                    capture_tokens: false,
                    scope_analysis: false,
                    maybe_syntax: None,
                })?;
                Code::String(parsed.transpile(&Default::default())?.text)
            } else {
                source
            };

            let module =
                ModuleSource::new(module_type, source.to_string()?.into(), &module_specifier);

            Ok(module)
        }

        futures::future::ready(load(module_specifier, embedded_dirs)).boxed_local()
    }
}
