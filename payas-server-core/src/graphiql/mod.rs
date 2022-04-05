use include_dir::{include_dir, Dir};
use std::path::Path;

static GRAPHIQL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../graphiql/build");

pub fn get_asset_bytes<P: AsRef<Path>>(file_name: P) -> Option<&'static [u8]> {
    GRAPHIQL_DIR.get_file(file_name).map(|file| file.contents())
}
