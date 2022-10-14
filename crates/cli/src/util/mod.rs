use anyhow::{anyhow, Result};
use payas_sql::{database_error::DatabaseError, Database};
use std::{
    fs::File,
    io::{self, stdin, stdout, Read, Write},
    path::Path,
};

pub(crate) mod watcher;

pub fn open_file_for_output(output: Option<&Path>) -> Result<Box<dyn Write>> {
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

pub fn open_database(database: Option<&str>) -> Result<Database, DatabaseError> {
    if let Some(database) = database {
        Ok(Database::from_db_url(database)?)
    } else {
        Ok(Database::from_env(Some(1))?)
    }
}