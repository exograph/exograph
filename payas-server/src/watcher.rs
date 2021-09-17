use std::path::Path;
use std::thread;

use anyhow::{bail, Result};

use crate::ServerLoopEvent;

use notify::{self, DebouncedEvent, RecursiveMode, Watcher};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

pub fn with_watch<T, STARTF, STOPF>(
    watched_path: impl AsRef<Path>,
    watch_delay: Duration,
    start: STARTF,
    mut stop: STOPF,
) -> Result<()>
where
    STARTF: Fn() -> Result<T>,
    STOPF: FnMut(&mut T),
{
    let rx = setup_watch(&watched_path, watch_delay)?;

    loop {
        let mut server = start()?;

        if !start_watching(&rx, &mut server, &mut stop)? {
            break;
        }
    }

    Ok(())
}

fn setup_watch(
    watched_path: impl AsRef<Path>,
    watch_delay: Duration,
) -> Result<Receiver<ServerLoopEvent>> {
    let (tx, rx) = mpsc::channel();

    let watched_path = watched_path.as_ref().to_path_buf();
    let tx2 = tx.clone();

    thread::spawn(move || -> Result<()> {
        let (watcher_tx, watcher_rx) = mpsc::channel();
        let mut watcher = notify::watcher(watcher_tx, watch_delay)?;
        watcher.watch(&watched_path, RecursiveMode::NonRecursive)?;

        loop {
            match watcher_rx.recv() {
                Ok(e) => {
                    if matches!(e, DebouncedEvent::Write(_)) {
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

fn start_watching<T, STOPF>(
    rx: &Receiver<ServerLoopEvent>,
    server: &mut T,
    mut stop: STOPF,
) -> Result<bool>
where
    STOPF: FnMut(&mut T),
{
    // Stop and restart the server initializtion loop when the model file is edited. Exit
    // the server loop when SIGINT is received.
    match rx.recv()? {
        ServerLoopEvent::FileChange => {
            println!("Restarting...");
            stop(server);
            Ok(true)
        }
        ServerLoopEvent::SigInt => {
            println!("Exiting");
            Ok(false)
        }
    }
}
