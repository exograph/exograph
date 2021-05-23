use std::{fs, path::Path};

use crate::ast::ast_types::*;

use nom::{error::VerboseError, IResult};

mod expression;
mod model;
mod util;

pub type PResult<I, O> = IResult<I, O, VerboseError<I>>;

pub fn parse_file<'a, P: AsRef<Path>>(path: P) -> AstSystem {
    let file_content = fs::read_to_string(path).unwrap();
    let parsed = model::system(&file_content);

    parsed
        .map(|(remaining, system)| {
            if remaining.is_empty() {
                system
            } else {
                panic!("Failed to parse some part of the file\n{}\n", remaining)
            }
        })
        .unwrap()
}
