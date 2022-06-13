use include_dir::{include_dir, Dir};
use std::path::Path;

use crate::{get_endpoint_http_path, get_playground_http_path};

static GRAPHIQL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../graphiql/build");

pub fn get_asset_bytes<P: AsRef<Path>>(file_name: P) -> Option<Vec<u8>> {
    GRAPHIQL_DIR
        .get_file(file_name)
        .map(|file| match file.contents_utf8() {
            Some(str) => {
                let str = str.replace("%%PLAYGROUND_URL%%", &get_playground_http_path());
                let str = str.replace("%%ENDPOINT_URL%%", &get_endpoint_http_path());

                str.as_bytes().to_owned()
            }
            None => file.contents().to_owned(),
        })
}
