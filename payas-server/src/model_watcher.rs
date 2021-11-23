use anyhow::Result;
use std::{fs, path::Path};

use super::watcher;

use std::time::Duration;

pub fn with_watch<T, STARTF, STOPF>(
    model_path: impl AsRef<Path>,
    watch_delay: Duration,
    start: STARTF,
    stop: STOPF,
) -> Result<()>
where
    STARTF: Fn(bool) -> Result<T>,
    STOPF: FnMut(&mut T),
{
    // We must canonicalize since we may be given a path such as "index.clay", and getting its parent
    // without canonicalizing would yield just "" and thus watches nothing.
    let model_path = fs::canonicalize(model_path).unwrap();

    // Recursively (done by `watcher::with_watch`) watch everything in the parent of the model file
    // But do not restart unless the changed path is a file (and not a directory) changes and that
    // file isn't an artifcact of our building (currently, we assume that we build files that end with ".bundle.js")
    let watched_paths = vec![model_path.parent().unwrap().to_path_buf()];

    fn should_restart(changed_path: &Path) -> bool {
        match fs::metadata(changed_path) {
            Ok(metadata) => {
                metadata.is_file() && !&changed_path.to_str().unwrap().ends_with(".bundle.js")
            }
            Err(_) => true, // An error occurred, perhaps a file/directory was removed, so we should restart to be safe
        }
    }

    watcher::with_watch(watched_paths, watch_delay, should_restart, start, stop)
}
