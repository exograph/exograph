// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::process::Command;

pub(crate) fn cmd(binary_name: &str) -> Command {
    // Pick up the current executable path and replace the file with the specified binary
    // This allows us to invoke `target/debug/exo test ...` or `target/release/exo test ...`
    // without updating the PATH env.
    // Thus, for the former invocation if the `binary_name` is `exo-server` the command will become
    // `<full-path-to>/target/debug/exo-server`
    let mut executable =
        std::env::current_exe().expect("Could not retrieve the current executable");
    executable.set_file_name(binary_name);
    Command::new(
        executable
            .to_str()
            .expect("Could not convert executable path to a string"),
    )
}
