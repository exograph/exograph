// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::io::stdin;

use clap::Arg;
use rand::Rng;

pub(super) fn generate_random_string() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(15)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub(crate) fn wait_for_enter(prompt: &str) -> std::io::Result<()> {
    println!("{prompt}");

    let mut line = String::new();
    stdin().read_line(&mut line)?;

    Ok(())
}

pub(crate) fn use_ir_arg() -> Arg {
    Arg::new("use-ir")
        .help("Use the IR file instead of the model file")
        .long("use-ir")
        .required(false)
        .num_args(0)
}
