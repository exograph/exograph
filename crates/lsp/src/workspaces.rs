use std::path::{Path, PathBuf};

use anyhow::Result;
use dashmap::DashMap;
use tower_lsp::lsp_types::Diagnostic;

use crate::workspace::{Document, Workspace};

#[derive(Debug)]
pub(crate) struct Workspaces {
    workspaces: DashMap<PathBuf, Workspace>,
}

impl Workspaces {
    pub fn new() -> Self {
        Self {
            workspaces: DashMap::new(),
        }
    }

    pub async fn insert_document(
        &self,
        path: PathBuf,
        document: Document,
    ) -> Result<Vec<(PathBuf, Option<i32>, Vec<Diagnostic>)>> {
        let matching_entry = self.workspaces.iter().find(|workspace| {
            let workspace_path = workspace.key();
            path.starts_with(workspace_path)
        });

        match matching_entry {
            Some(workspace) => workspace.insert_document(path.clone(), document).await,
            None => {
                let workspace_root = Self::workspace_root_for_document_path(&path);
                if let Some(workspace_root) = workspace_root {
                    let new_workspace = Workspace::new(workspace_root.clone());
                    let diagnostics = new_workspace.insert_document(path.clone(), document).await;

                    let _ = self.workspaces.insert(workspace_root, new_workspace);
                    diagnostics
                } else {
                    tracing::error!("No workspace root found for document: {}", path.display());
                    Ok(vec![])
                }
            }
        }
    }

    fn workspace_root_for_document_path(document_path: &Path) -> Option<PathBuf> {
        // Walk up the directory tree until we find the "src" directory
        let mut current_path = document_path.to_path_buf();
        while !current_path.ends_with("src") {
            current_path = current_path.parent().unwrap().to_path_buf();
        }
        Some(current_path.parent().unwrap().to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_root_for_document_path() {
        assert_eq!(
            Workspaces::workspace_root_for_document_path(&PathBuf::from(
                "/Users/name/test/src/index.exo"
            )),
            Some(PathBuf::from("/Users/name/test"))
        );
    }

    #[tokio::test]
    async fn insert_invalid_document() {
        let workspaces = Workspaces::new();

        let diagnostics = workspaces
            .insert_document(
                PathBuf::from("/Users/name/test/src/index.exo"),
                Document::new("module {}".to_string(), None),
            )
            .await
            .unwrap();

        let (path, _, diagnostics) = diagnostics.first().unwrap();
        println!("diagnostics: {:?}", diagnostics);
        assert_eq!(path, &PathBuf::from("/Users/name/test/src/index.exo"));
        assert_eq!(diagnostics.len(), 1);
    }
}
