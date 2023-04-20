// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use lambda_runtime::Error;
use lambda_runtime::LambdaEvent;

use serde_json::Value;
use server_aws_lambda::resolve;

use std::sync::Arc;

/// Run the server in production mode with a compiled exo_ir file
#[tokio::main]
async fn main() -> Result<(), Error> {
    let system_resolver = Arc::new(server_common::init());

    let module = lambda_runtime::service_fn(|event: LambdaEvent<Value>| async {
        resolve(event, system_resolver.clone()).await
    });

    lambda_runtime::run(module).await?;

    Ok(())
}
