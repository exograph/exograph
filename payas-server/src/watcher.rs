use anyhow::{bail, Result};
use std::path::Path;
use std::thread;

use crate::ServerLoopEvent;

use notify::{self, DebouncedEvent, RecursiveMode, Watcher};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

pub fn with_watch<T, STARTF, STOPF>(
    watched_paths: Vec<impl AsRef<Path> + Send + 'static>,
    watch_delay: Duration,
    start: STARTF,
    mut stop: STOPF,
) -> Result<()>
where
    STARTF: Fn(bool) -> Result<T>,
    STOPF: FnMut(&mut T),
{
    let rx = setup_watch(watched_paths, watch_delay)?;

    let mut restart = false;

    loop {
        let server = start(restart);
        restart = true;

        let cont = wait_for_change(&rx)?;

        if cont {
            if let Ok(mut server) = server {
                stop(&mut server);
            }
        } else {
            break;
        }
    }

    Ok(())
}

fn setup_watch(
    watched_paths: Vec<impl AsRef<Path> + Send + 'static>,
    watch_delay: Duration,
) -> Result<Receiver<ServerLoopEvent>> {
    let (tx, rx) = mpsc::channel();

    //let watched_path = watched_path.as_ref().to_path_buf();
    let tx2 = tx.clone();

    thread::spawn(move || -> Result<()> {
        let (watcher_tx, watcher_rx) = mpsc::channel();
        let mut watcher = notify::watcher(watcher_tx, watch_delay)?;

        // for entry in globwalk::GlobWalkerBuilder::from_patterns(
        //     watched_path.parent().unwrap(),
        //     &["*", "!*.bundle.*"],
        // )
        // .build()?
        // {
        //     watcher.watch(entry?.path(), RecursiveMode::NonRecursive)?;
        // }
        for watched_path in watched_paths.iter() {
            watcher.watch(watched_path, RecursiveMode::NonRecursive)?;
        }

        loop {
            match watcher_rx.recv() {
                Ok(e) => {
                    if matches!(e, DebouncedEvent::Write(_))
                        || matches!(e, DebouncedEvent::Remove(_))
                    {
                        tx.send(ServerLoopEvent::FileChange)?;
                    }
                }
                Err(e) => bail!(e),
            }
        }
    });

    // Watch for ctrl-c (SIGINT)
    ctrlc::set_handler(move || {
        tx2.send(ServerLoopEvent::SigInt).unwrap();
    })?;

    Ok(rx)
}

fn wait_for_change(rx: &Receiver<ServerLoopEvent>) -> Result<bool> {
    // Stop and restart the server initializtion loop when the model file is edited. Exit
    // the server loop when SIGINT is received.
    match rx.recv()? {
        ServerLoopEvent::FileChange => {
            println!("Restarting...");
            Ok(true)
        }
        ServerLoopEvent::SigInt => {
            println!("Exiting");
            Ok(false)
        }
    }
}
