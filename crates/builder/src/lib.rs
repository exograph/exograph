// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#[cfg(not(target_family = "wasm"))]
use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
    path::Path,
};

#[cfg(not(target_family = "wasm"))]
use std::env::current_exe;

use codemap::CodeMap;
use codemap_diagnostic::{ColorConfig, Diagnostic, Emitter, Level};
use core_plugin_interface::interface::{LibraryLoadingError, SubsystemBuilder};
use core_plugin_shared::{
    serializable_system::SerializableSystem, trusted_documents::TrustedDocuments,
};
use error::ParserError;

#[cfg(not(target_family = "wasm"))]
use colored::Colorize;

mod builder;
pub mod error;
pub mod parser;
pub mod typechecker;
mod util;

use core_model_builder::{
    ast::{
        self,
        ast_types::{AstSystem, Untyped},
    },
    error::ModelBuildingError,
};
#[cfg(not(target_family = "wasm"))]
use regex::Regex;
use serde::{Deserialize, Serialize};

#[cfg(not(target_family = "wasm"))]
/// Build a model system from a exo file
pub async fn build_system(
    model_file: impl AsRef<Path>,
    trusted_documents_dir: Option<impl AsRef<Path>>,
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
) -> Result<SerializableSystem, ParserError> {
    let file_content = fs::read_to_string(model_file.as_ref())?;
    let mut codemap = CodeMap::new();

    codemap.add_file(model_file.as_ref().display().to_string(), file_content);

    let trusted_documents = trusted_documents_dir
        .map(|dir| load_trusted_documents(dir))
        .unwrap_or(Ok(TrustedDocuments::all()))?;

    match build_from_ast_system(
        parser::parse_file(&model_file, &mut codemap),
        trusted_documents,
        static_builders,
    )
    .await
    {
        Ok(bytes) => Ok(bytes),
        Err(err) => {
            StderrReporter {}.emit(&codemap, &err);
            Err(err)
        }
    }
}

// Can we expose this only for testing purposes?
// #[cfg(test)]
pub async fn build_system_from_str(
    model_str: &str,
    file_name: String,
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
) -> Result<SerializableSystem, ParserError> {
    build_system_from_str_with_reporting(
        model_str,
        file_name,
        static_builders,
        &mut StderrReporter {},
    )
    .await
}

pub async fn build_system_from_str_with_reporting(
    model_str: &str,
    file_name: String,
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
    reporter: &mut impl ErrorReporter,
) -> Result<SerializableSystem, ParserError> {
    let mut codemap = CodeMap::new();
    codemap.add_file(file_name.clone(), model_str.to_string());

    match build_from_ast_system(
        parser::parse_str(model_str, &mut codemap, &file_name),
        TrustedDocuments::all(),
        static_builders,
    )
    .await
    {
        Ok(bytes) => Ok(bytes),
        Err(err) => {
            reporter.emit(&codemap, &err);
            Err(err)
        }
    }
}

pub fn load_subsystem_builders(
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
) -> Result<Vec<Box<dyn SubsystemBuilder + Send + Sync>>, LibraryLoadingError> {
    #[allow(unused_mut)]
    let mut subsystem_builders = static_builders;

    #[cfg(not(target_family = "wasm"))]
    {
        let mut dir = current_exe()?;
        dir.pop();

        let pattern = format!(
            "{}(.+)_model_builder_dynamic\\{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        );
        let pattern = Regex::new(&pattern).unwrap();

        for entry in dir.read_dir()?.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                let captures = pattern.captures(file_name);
                if let Some(captures) = captures {
                    let subsystem_id = captures.get(1).unwrap().as_str();

                    // First see if we have already loaded a static builder
                    let builder = subsystem_builders
                        .iter()
                        .find(|builder| builder.id() == subsystem_id);

                    if builder.is_none() {
                        // Then try to load a dynamic builder
                        subsystem_builders.push(
                            core_plugin_interface::interface::load_subsystem_builder(
                                &entry.path(),
                            )?,
                        );
                    };
                }
            }
        }
    }

    Ok(subsystem_builders)
}

async fn build_from_ast_system(
    ast_system: Result<AstSystem<Untyped>, ParserError>,
    trusted_documents: TrustedDocuments,
    static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>>,
) -> Result<SerializableSystem, ParserError> {
    let subsystem_builders = load_subsystem_builders(static_builders)
        .map_err(|e| ParserError::Generic(format!("{e}")))?;

    let ast_system = ast_system?;

    let typechecked_system = typechecker::build(&subsystem_builders, ast_system)?;

    Ok(builder::build(&subsystem_builders, typechecked_system, trusted_documents).await?)
}

#[cfg(not(target_family = "wasm"))]
fn load_trusted_documents(
    trusted_documents_dir: impl AsRef<Path>,
) -> Result<TrustedDocuments, ParserError> {
    fn from_file(path: &std::path::Path) -> Result<HashMap<String, String>, ParserError> {
        // Parse the file with the format (generated by graphql codegen):
        // {
        //    <hash>: <document>,
        //    ...
        // }
        fn from_file_simple_map(
            path: &std::path::Path,
        ) -> Result<HashMap<String, String>, ParserError> {
            let path = File::open(path)?;
            let reader = BufReader::new(path);
            serde_json::from_reader(reader)
                .map_err(|e| ParserError::InvalidTrustedDocumentFormat(format!("{e}")))
        }

        // Parse the file with the format (generated by Apollo generate-persisted-query-manifest):
        // {
        //     "format": "apollo-persisted-query-manifest", // ignored
        //     "version": 1, // ignored
        //     "operations": [
        //       {
        //         "id": <hash>,
        //         "name": "...", // ignored
        //         "type": "mutation", // ignored
        //         "body": <query>
        //       },
        //       ...
        //     ]
        // }
        fn from_file_apollo_persisted_query_manifest(
            path: &std::path::Path,
        ) -> Result<HashMap<String, String>, ParserError> {
            let file = File::open(path)?;
            let reader = BufReader::new(file);

            #[derive(serde::Deserialize)]
            struct Operation {
                id: String,
                body: String,
            }

            #[derive(serde::Deserialize)]
            struct Manifest {
                operations: Vec<Operation>,
            }

            let manifest: Manifest = serde_json::from_reader(reader)
                .map_err(|e| ParserError::InvalidTrustedDocumentFormat(format!("{e}")))?;

            Ok(manifest
                .operations
                .into_iter()
                .map(|op| (op.id, op.body))
                .collect())
        }

        println!("\tLoading trusted documents from: {}", path.display());

        from_file_simple_map(path)
            .or_else(|_| from_file_apollo_persisted_query_manifest(path))
            .map_err(|e| ParserError::InvalidTrustedDocumentFormat(format!("{e}")))
    }

    fn from_dir(path: &std::path::Path) -> Result<HashMap<String, String>, ParserError> {
        let mut trusted_documents_map = HashMap::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let map = if path.is_file() && path.extension().unwrap() == "json" {
                from_file(&path)
            } else if entry.file_type()?.is_dir() {
                from_dir(&path)
            } else {
                println!("\tIgnoring file: {}", path.display());
                Ok(HashMap::new())
            }?;
            trusted_documents_map.extend(map);
        }

        if trusted_documents_map.is_empty() {
            // If the user has created "trusted_documents" directory, but it's empty, warn the user,
            // since no query or mutation will be trusted and thus allowed to run.
            println!(
                "{}",
                format!(
                    "No trusted documents found in directory: `{}`:",
                    path.display()
                )
                .red()
            );

            println!(
                "\t{}",
                "You will not be able to execute any queries or mutations.".red()
            );

            println!(
                "\t{}",
                format!(
                    "Either delete the {} directory or add trusted documents to it.",
                    path.display()
                )
                .red()
            );
        }
        Ok(trusted_documents_map)
    }

    if Path::exists(trusted_documents_dir.as_ref()) {
        println!(
            "Found trusted documents directory: {}",
            trusted_documents_dir.as_ref().display()
        );
        let trusted_documents_map = from_dir(trusted_documents_dir.as_ref())?;
        Ok(TrustedDocuments::from_map(trusted_documents_map, false))
    } else {
        Ok(TrustedDocuments::all())
    }
}

pub trait ErrorReporter {
    fn emit(&mut self, codemap: &CodeMap, err: &ParserError);
}

struct StderrReporter;

impl ErrorReporter for StderrReporter {
    fn emit(&mut self, codemap: &CodeMap, err: &ParserError) {
        let mut emitter = Emitter::stderr(ColorConfig::Always, Some(codemap));
        emit_diagnostics(err, &mut emitter, codemap, None);
    }
}

/// Collect diagnostics and display string from the parser so an external reporter can use them
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ExternalReporter {
    diagnostics: Vec<report::Diagnostic>,
    #[serde(rename = "displayString")]
    display_string: String,
}

impl ErrorReporter for ExternalReporter {
    fn emit(&mut self, codemap: &CodeMap, err: &ParserError) {
        let mut buffer: Vec<u8> = vec![];
        {
            let mut emitter = Emitter::vec(&mut buffer, Some(codemap));
            emit_diagnostics(err, &mut emitter, codemap, Some(&mut self.diagnostics));
        }
        self.display_string = String::from_utf8(buffer).unwrap();
    }
}

pub mod report {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Diagnostic {
        pub spans: Vec<Span>,
        pub message: String,
        #[serde(rename = "isError")]
        pub is_error: bool,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Span {
        #[serde(rename = "fileName")]
        pub file_name: String,
        pub start: Position,
        pub end: Position,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Position {
        pub line: usize,
        pub column: usize,
    }
}

fn emit_diagnostics(
    err: &ParserError,
    emitter: &mut Emitter,
    codemap: &CodeMap,
    report_collector: Option<&mut Vec<report::Diagnostic>>,
) {
    fn collect_diagnostics(
        diagnostics: &[Diagnostic],
        codemap: &CodeMap,
        report_collector: Option<&mut Vec<report::Diagnostic>>,
    ) {
        if let Some(report_collector) = report_collector {
            diagnostics.iter().for_each(|diagnostic| {
                let report_spans = diagnostic
                    .spans
                    .iter()
                    .map(|span_label| {
                        let span_loc = codemap.look_up_span(span_label.span);
                        report::Span {
                            file_name: span_loc.file.name().to_string(),
                            start: report::Position {
                                line: span_loc.begin.line,
                                column: span_loc.begin.column,
                            },
                            end: report::Position {
                                line: span_loc.end.line,
                                column: span_loc.end.column,
                            },
                        }
                    })
                    .collect();

                report_collector.push(report::Diagnostic {
                    spans: report_spans,
                    message: diagnostic.message.clone(),
                    is_error: diagnostic.level == Level::Error,
                });
            });
        }
    }

    match err {
        ParserError::Diagnosis(diagnostics) => {
            collect_diagnostics(diagnostics, codemap, report_collector);
            emitter.emit(diagnostics);
        }
        ParserError::ModelBuildingError(ModelBuildingError::Diagnosis(diagnostics)) => {
            collect_diagnostics(diagnostics, codemap, report_collector);
            emitter.emit(diagnostics);
        }
        ParserError::ModelBuildingError(ModelBuildingError::ExternalResourceParsing(e)) => {
            // This is an error in a JavaScript/TypeScript file, so we
            // have emit it directly to stderr (can't use the emitter, which is tied to exo sources)
            emitter.emit(&[Diagnostic {
                level: Level::Error,
                code: None,
                message: e.to_string(),
                spans: vec![],
            }]);
        }
        _ => {
            emitter.emit(&[Diagnostic {
                level: Level::Error,
                code: None,
                message: format!("{err}"),
                spans: vec![],
            }]);
        }
    }
}
