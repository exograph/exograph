use anyhow::Result;
use globwalk;
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
    // We must canonicalize since we may be handed path such as "index.clay", and getting its parent
    // without canonicalizing would yield just "" and thus watches nothing.
    let model_path = fs::canonicalize(model_path).unwrap();

    // Watch everything in the parent of the model file, expept for the artifacts we generate (the bundle files)
    // under the assumption that users won't include files whose names match the *.bundle.* pattern.
    // An alternative would be to parse the model file and look for the artifacts used, but that will
    // require parsing the js/ts files referred to in the model files recursively to gather all the dependencies.
    let watched_paths = move || {
        let parent_path = model_path.parent().unwrap();
        let mut paths: Vec<_> =
            globwalk::GlobWalkerBuilder::from_patterns(parent_path, &["**/*", "!*.bundle.*"])
                .build()
                .unwrap()
                .filter_map(|file| file.ok().map(|f| f.path().to_owned()))
                .collect();
        paths.push(parent_path.to_path_buf());
        paths
    };

    watcher::with_watch(watched_paths, watch_delay, start, stop)
}
