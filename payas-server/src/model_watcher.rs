use anyhow::Result;
use globwalk;
use std::path::Path;

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
    let model_path = model_path.as_ref().to_path_buf();

    // Watch everything in the parent of the model file, expept for the artifacts we generate (the bundle files)
    // under the assumption that users won't include files whose names match the *.bundle.* pattern.
    // An alternative would be to parse the model file and look for the artifacts used, but that will
    // require parsing the js/ts files referred to in the model files recursively to gather all the dependencies.
    let watched_paths: Vec<_> = globwalk::GlobWalkerBuilder::from_patterns(
        model_path.parent().unwrap(),
        &["*", "!*.bundle.*"],
    )
    .build()?
    .filter_map(|file| file.ok().map(|f| f.path().to_owned()))
    .collect();

    watcher::with_watch(watched_paths, watch_delay, start, stop)
}
