use include_dir::{include_dir, Dir};
use std::path::Path;

use crate::root_resolver::{get_endpoint_http_path, get_playground_http_path};

static GRAPHIQL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../graphiql/build");

pub fn get_asset_bytes<P: AsRef<Path>>(file_name: P) -> Option<Vec<u8>> {
    GRAPHIQL_DIR.get_file(file_name.as_ref()).map(|file| {
        if file_name.as_ref() == Path::new("index.html") {
            let str = file
                .contents_utf8()
                .expect("index.html for playground should be utf8");
            let str = str.replace("%%PLAYGROUND_URL%%", &get_playground_http_path());
            let str = str.replace("%%ENDPOINT_URL%%", &get_endpoint_http_path());
            str.as_bytes().to_owned()
        } else {
            file.contents().to_owned()
        }
    })
}
