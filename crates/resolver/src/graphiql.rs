// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use common::env_const::EXO_INTROSPECTION_LIVE_UPDATE;
use include_dir::{include_dir, Dir};
use std::path::Path;

use crate::root_resolver::{get_endpoint_http_path, get_playground_http_path};

static GRAPHIQL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../graphiql/build");

pub fn get_asset_bytes<P: AsRef<Path>>(file_name: P) -> Option<Vec<u8>> {
    let enable_introspection_live_update =
        std::env::var(EXO_INTROSPECTION_LIVE_UPDATE).unwrap_or_else(|_| "false".to_string());
    // Normalize the OIDC URL to remove the trailing slash, if any
    let oidc_url = std::env::var("EXO_OIDC_URL")
        .map(|s| s.trim_end_matches('/').to_owned())
        .unwrap_or_else(|_| "".to_owned());

    GRAPHIQL_DIR.get_file(file_name.as_ref()).map(|file| {
        if file_name.as_ref() == Path::new("index.html") {
            let str = file
                .contents_utf8()
                .expect("index.html for playground should be utf8");
            let str = str.replace("%%PLAYGROUND_URL%%", &get_playground_http_path());

            // If the server is running in the playground mode, then use the endpoint URL from the
            // environment variable (such as "http://<remote-server-address>/graphql"). Otherwise,
            // use the default endpoint relative path (such as "/graphql").
            let str = match std::env::var("_EXO_PLAYGROUND_ENDPOINT_URL") {
                Ok(url) => str.replace("%%ENDPOINT_URL%%", &url),
                Err(_) => str.replace("%%ENDPOINT_URL%%", &get_endpoint_http_path()),
            };

            let str = str.replace("%%SCHEMA_URL%%", &get_endpoint_http_path());

            let str = str.replace(
                "%%ENABLE_INTROSPECTION_LIVE_UPDATE%%",
                &enable_introspection_live_update,
            );

            let str = str.replace("%%OIDC_URL%%", &oidc_url);

            str.as_bytes().to_owned()
        } else {
            file.contents().to_owned()
        }
    })
}
