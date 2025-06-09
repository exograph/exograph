// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{Result, anyhow};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Write, stdin, stdout},
    path::Path,
};

pub(crate) mod watcher;

pub fn open_file_for_output(output: Option<&Path>, yes: bool) -> Result<Box<dyn Write + Send>> {
    if let Some(output) = output {
        if output.exists() && !yes {
            print!(
                "File `{}` already exists. Overwrite? [y/N]: ",
                output.display()
            );
            io::stdout().flush()?;

            let mut input = String::new();
            BufReader::new(stdin()).read_line(&mut input)?;
            let char = input.trim().chars().next().unwrap_or('n');

            if char != 'y' {
                return Err(anyhow!("Not overwriting file"));
            }
        }

        Ok(Box::new(File::create(output)?))
    } else {
        Ok(Box::new(stdout()))
    }
}
