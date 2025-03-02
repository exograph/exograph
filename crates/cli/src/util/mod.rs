// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::{self, stdin, stdout, Read, Write},
    path::Path,
};

pub(crate) mod watcher;

pub fn open_file_for_output(output: Option<&Path>) -> Result<Box<dyn Write + Send>> {
    if let Some(output) = output {
        if output.exists() {
            print!(
                "File `{}` already exists. Overwrite? [y/N]: ",
                output.display()
            );
            io::stdout().flush()?;

            let char = stdin().bytes().next().unwrap().unwrap();

            if char != b'y' {
                return Err(anyhow!("Not overwriting file"));
            }
        }

        Ok(Box::new(File::create(output)?))
    } else {
        Ok(Box::new(stdout()))
    }
}
