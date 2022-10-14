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
        _is_main: bool,
    ) -> Result<ModuleSpecifier, deno_core::anyhow::Error> {
        Ok(resolve_import(specifier, referrer)?)
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<ModuleSpecifier>,
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

            pub fn into_bytes(self) -> Result<Box<[u8]>, AnyError> {
                match self {
                    Code::Slice(slice) => Ok(Box::from(slice)),
                    Code::Vec(vec) => Ok(Box::from(vec)),
                    Code::String(s) => Ok(s.into_bytes().into_boxed_slice()),
                }
            }
        }

        let module_specifier = module_specifier.clone();
        let embedded_dirs = self.embedded_dirs.clone();

        // adapted from https://github.com/denoland/deno/blob/94d369ebc65a55bd9fbf378a765c8ed88a4efe2c/core/examples/ts_module_loader.rs
        async move {
            let (source, media_type): (Code, MediaType) = match module_specifier.scheme() {
                "http" | "https" => {
                    let path = PathBuf::from(module_specifier.path());

                    let mut writer = Vec::new();
                    let res = http_req::request::get(&module_specifier, &mut writer)?;

                    if !res.status_code().is_success() {
                        bail!("Failed to fetch {}: {}", module_specifier, res.reason())
                    }

                    (Code::Vec(writer), MediaType::from(&path))
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

                    (code, MediaType::from(&path))
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

                    (code, MediaType::from(&path))
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

            let module = ModuleSource {
                code: source.into_bytes()?,
                module_type,
                module_url_specified: module_specifier.to_string(),
                module_url_found: module_specifier.to_string(),
            };

            Ok(module)
        }
        .boxed_local()
    }
}
