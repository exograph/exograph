use anyhow::Result;
use builder::{build_from_ast_system, error::ParserError, parser, FileSystem};
use codemap::CodeMap;
use core_plugin_shared::trusted_documents::TrustedDocuments;
use std::{
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
};

use dashmap::DashMap;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use core_model_builder::{error::ModelBuildingError, plugin::BuildMode};
use core_plugin_interface::interface::SubsystemBuilder;

#[derive(Debug)]
pub(crate) struct Workspace {
    root: PathBuf,
    documents: DashMap<PathBuf, Document>,
}

impl FileSystem for Workspace {
    fn read_file(&self, path: impl AsRef<Path>) -> Result<String, std::io::Error> {
        let path = path.as_ref();
        let document = self.documents.get(path).ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", path.display()),
        ))?;
        Ok(document.content.clone())
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        self.documents.contains_key(path.as_ref())
    }
}

impl Workspace {
    pub fn new(root: PathBuf) -> Self {
        let workspace = Self {
            root,
            documents: DashMap::new(),
        };
        let _ = workspace.seed_documents(&workspace.root);
        workspace
    }

    fn seed_documents(&self, dir: impl AsRef<Path>) -> Result<(), ParserError> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension() == Some(std::ffi::OsStr::new("exo")) {
                let content = std::fs::read_to_string(&path)?;
                self.documents.insert(path, Document::new(content, None));
            } else if entry.file_type()?.is_dir() {
                self.seed_documents(&path)?;
            }
        }

        Ok(())
    }

    async fn build(&self) -> Result<Vec<(PathBuf, Option<i32>, Vec<Diagnostic>)>> {
        let index_file = self.root.join("src").join("index.exo");

        let static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>> = vec![
            Box::new(postgres_builder::PostgresSubsystemBuilder::default()),
            Box::new(deno_builder::DenoSubsystemBuilder::default()),
            Box::new(wasm_builder::WasmSubsystemBuilder::default()),
        ];

        let file_content = self.read_file(&index_file)?;
        let mut codemap = CodeMap::new();
        codemap.add_file(index_file.display().to_string(), file_content);

        use futures::FutureExt;

        // Build panics (mostly indirectly through unwraps), which typically happens
        // when the the user is still typing and likely to be fixed in a few additional keystrokes.
        // While we will continue to fix those panics, until we have fixed them, we can use catch_unwind to
        // handle them and not make the LSP crash.
        let build_result = AssertUnwindSafe(build_from_ast_system(
            parser::parse_file(&index_file, self, &mut codemap),
            TrustedDocuments::all(),
            None,
            static_builders,
            BuildMode::CheckOnly, // In the LSP mode, we do not want to cause any side effects such as generating TypeScript code
        ))
        .catch_unwind()
        .await;

        match build_result {
            Ok(Ok(_)) => {
                let diagnostics = self
                    .documents
                    .iter()
                    .map(|entry| {
                        let path = entry.key().clone();
                        let version = entry.value().version;
                        (path, version, vec![])
                    })
                    .collect::<Vec<_>>();
                Ok(diagnostics)
            }
            Ok(Err(err)) => match err {
                ParserError::Diagnosis(diagnosis) => {
                    let diagnostics: Vec<_> = self.compute_diagnostics(diagnosis, &codemap);
                    Ok(diagnostics)
                }
                ParserError::ModelBuildingError(parser_error) => {
                    if let ModelBuildingError::Diagnosis(diagnosis) = parser_error {
                        let diagnostics: Vec<_> = self.compute_diagnostics(diagnosis, &codemap);
                        Ok(diagnostics)
                    } else {
                        Ok(self.generic_diagnostic(parser_error.to_string()))
                    }
                }
                err => Ok(self.generic_diagnostic(err.to_string())),
            },
            Err(err) => {
                eprintln!("build_result: {:?}", err);
                Ok(self.generic_diagnostic("Unknown error while building model".to_string()))
            }
        }
    }

    pub async fn insert_document(
        &self,
        path: PathBuf,
        document: Document,
    ) -> Result<Vec<(PathBuf, Option<i32>, Vec<Diagnostic>)>> {
        self.documents.insert(path.clone(), document);

        let diagnostics = self.build().await?;

        Ok(diagnostics)
    }

    pub fn compute_diagnostics(
        &self,
        exo_diagnostics: Vec<codemap_diagnostic::Diagnostic>,
        codemap: &CodeMap,
    ) -> Vec<(PathBuf, Option<i32>, Vec<Diagnostic>)> {
        exo_diagnostics
            .into_iter()
            .map(|d| {
                if d.spans.is_empty() {
                    (
                        self.root.join("src").join("index.exo"),
                        None,
                        vec![Diagnostic {
                            message: d.message,
                            severity: Some(DiagnosticSeverity::ERROR),
                            ..Default::default()
                        }],
                    )
                } else {
                    let span = d.spans[0].span;
                    let loc = codemap.look_up_span(span);

                    let message = if d.spans.len() == 1 {
                        d.message
                    } else {
                        format!("{} ({} more)", d.message, d.spans.len() - 1)
                    };

                    let path = PathBuf::from(loc.file.name());
                    let version = self.documents.get(&path).unwrap().version;
                    (
                        path,
                        version,
                        vec![Diagnostic {
                            range: Range {
                                start: Position {
                                    line: loc.begin.line as u32,
                                    character: loc.begin.column as u32,
                                },
                                end: Position {
                                    line: loc.end.line as u32,
                                    character: loc.end.column as u32,
                                },
                            },
                            message,
                            severity: Some(DiagnosticSeverity::ERROR),
                            ..Default::default()
                        }],
                    )
                }
            })
            .collect()
    }

    pub fn generic_diagnostic(
        &self,
        message: String,
    ) -> Vec<(PathBuf, Option<i32>, Vec<Diagnostic>)> {
        vec![(
            self.root.join("src").join("index.exo"),
            None,
            vec![Diagnostic {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
                message,
                severity: Some(DiagnosticSeverity::ERROR),
                ..Default::default()
            }],
        )]
    }
}

#[derive(Debug)]
pub struct Document {
    content: String,
    version: Option<i32>,
}

impl Document {
    pub fn new(content: String, version: Option<i32>) -> Self {
        Self { content, version }
    }
}
