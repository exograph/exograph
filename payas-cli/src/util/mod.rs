use anyhow::{anyhow, Result};
use std::{
    fs::File,
    io::{self, stdin, Read, Write},
    path::Path,
};

pub(crate) mod watcher;

pub fn open_file_for_output(output: &Path) -> Result<File> {
    if output.exists() {
        print!(
            "File `{}` already exists. Overwrite? [y/N]: ",
            output.display()
        );
        io::stdout().flush()?;

        let char = stdin().bytes().next().unwrap().unwrap();

        if char == b'y' {
            Ok(File::create(output)?)
        } else {
            Err(anyhow!("Not overwriting file"))
        }
    } else {
        Ok(File::create(output)?)
    }
}
