use std::path::PathBuf;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializeParams, InitializeResult,
    InitializedParams, MessageType, SaveOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions,
};
use tower_lsp::{Client, LanguageServer};
use url::Url;

use crate::workspace::Document;
use crate::workspaces::Workspaces;

#[derive(Debug)]
pub(crate) struct Backend {
    client: Client,
    workspaces: Workspaces,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            workspaces: Workspaces::new(),
        }
    }

    pub async fn on_change(&self, path: PathBuf, document: Document) -> Result<()> {
        let diagnostics = self
            .workspaces
            .insert_document(path, document)
            .await
            .map_err(|e| {
                tracing::error!("Error inserting document: {}", e);
                tower_lsp::jsonrpc::Error::parse_error()
            })?;

        for (path, version, diagnostics) in diagnostics {
            self.client
                .publish_diagnostics(Url::from_file_path(path).unwrap(), diagnostics, version)
                .await;
        }

        Ok(())
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "Exograph Language Server".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            offset_encoding: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        tracing::info!("server initialized!");
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let path = PathBuf::from(params.text_document.uri.path());
        let _ = self
            .on_change(
                path,
                Document::new(
                    params.text_document.text,
                    Some(params.text_document.version),
                ),
            )
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let path = PathBuf::from(params.text_document.uri.path());
        let _ = self
            .on_change(
                path,
                Document::new(
                    params
                        .content_changes
                        .into_iter()
                        .last()
                        .expect("no content changes")
                        .text,
                    Some(params.text_document.version),
                ),
            )
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let content = params.text;

        match content {
            Some(content) => {
                self.on_change(
                    PathBuf::from(params.text_document.uri.path()),
                    Document::new(content, None),
                )
                .await
                .unwrap();
            }
            None => {
                _ = self.client.semantic_tokens_refresh().await;
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        tracing::debug!("Text document did close {:?}", params.text_document.uri);
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        tracing::debug!(
            "Text document did change watched files {:?}",
            params.changes
        );
    }
}
