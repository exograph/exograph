// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    env,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use common::logging_tracing;
use tokio::sync::{
    Mutex,
    broadcast::{Receiver, Sender},
};

use commands::{
    build::BuildCommandDefinition,
    command::{CommandDefinition, SubcommandDefinition},
    deploy,
    dev::DevCommandDefinition,
    graphql,
    new::NewCommandDefinition,
    playground::PlaygroundCommandDefinition,
    schema,
    test::TestCommandDefinition,
    update::UpdateCommandDefinition,
    yolo::YoloCommandDefinition,
};

mod commands;
mod config;
mod util;

lazy_static::lazy_static! {
    pub static ref SIGINT: (Sender<()>, Mutex<Receiver<()>>) = {
        let (tx, rx) = tokio::sync::broadcast::channel(1);
        (tx, Mutex::new(rx))
    };
}

pub static EXIT_ON_SIGINT: AtomicBool = AtomicBool::new(true);

#[tokio::main]
async fn main() -> Result<()> {
    logging_tracing::init().await?;

    // register a sigint handler
    ctrlc::set_handler(move || {
        // set SIGINT event when receiving signal
        let _ = SIGINT.0.send(());

        // exit if EXIT_ON_SIGINT is set
        // code may set this to be false if they have resources to
        // clean up before exiting
        if EXIT_ON_SIGINT.load(Ordering::SeqCst) {
            std::process::exit(0);
        }
    })
    .expect("Error setting Ctrl-C handler");

    let subcommand_definition = SubcommandDefinition::new(
        "Exograph",
        "Command line interface for Exograph",
        vec![
            Box::new(NewCommandDefinition {}),
            Box::new(YoloCommandDefinition {}),
            Box::new(DevCommandDefinition {}),
            Box::new(BuildCommandDefinition {}),
            Box::new(deploy::command_definition()),
            Box::new(schema::command_definition()),
            Box::new(graphql::command_definition()),
            Box::new(PlaygroundCommandDefinition {}),
            Box::new(UpdateCommandDefinition {}),
            Box::new(TestCommandDefinition {}),
        ],
    );

    let command = subcommand_definition
        .command()
        .version(env!("CARGO_PKG_VERSION"));

    let matches = command.get_matches();

    let config = config::load_config()?;

    exo_deno::initialize();

    subcommand_definition.execute(&matches, &config).await
}
