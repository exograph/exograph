// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::Result;
use server::Backend;
use tower_lsp::{LspService, Server};

mod server;
mod trace_setup;
mod workspace;
mod workspaces;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = trace_setup::setup();

    start().await?;

    Ok(())
}

pub async fn start() -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;

    Ok(())
}
