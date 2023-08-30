// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use include_dir::{include_dir, Dir};
use std::path::Path;

use crate::{
    root_resolver::{get_endpoint_http_path, get_playground_http_path},
    system_loader::EXO_INTROSPECTION_LIVE_UPDATE,
};

static GRAPHIQL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../graphiql/build");

pub fn get_asset_bytes<P: AsRef<Path>>(file_name: P) -> Option<Vec<u8>> {
    let enable_introspection_live_update =
        std::env::var(EXO_INTROSPECTION_LIVE_UPDATE).unwrap_or_else(|_| "false".to_string());
    let jwks_endpoint = std::env::var("EXO_JWKS_ENDPOINT").unwrap_or_else(|_| "".to_string());

    GRAPHIQL_DIR.get_file(file_name.as_ref()).map(|file| {
        if file_name.as_ref() == Path::new("index.html") {
            let str = file
                .contents_utf8()
                .expect("index.html for playground should be utf8");
            let str = str.replace("%%PLAYGROUND_URL%%", &get_playground_http_path());
            let str = str.replace("%%ENDPOINT_URL%%", &get_endpoint_http_path());
            let str = str.replace(
                "%%ENABLE_INTROSPECTION_LIVE_UPDATE%%",
                &enable_introspection_live_update,
            );

            let str = {
                let jwks_base_url = if jwks_endpoint.is_empty() {
                    ""
                } else {
                    let jwks_path = "/.well-known/jwks.json";

                    if jwks_endpoint.ends_with(jwks_path) {
                        &jwks_endpoint[..jwks_endpoint.len() - jwks_path.len()]
                    } else {
                        &jwks_endpoint
                    }
                };

                str.replace("%%JWKS_BASE_URL%%", jwks_base_url)
            };

            str.as_bytes().to_owned()
        } else {
            file.contents().to_owned()
        }
    })
}
